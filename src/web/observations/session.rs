//! One serialized observation session; waiting HTTP demand shares its capture.
use super::memory::{Budget, Charge, MIB, ResponseBytes};
use super::socket::Connection;
use super::wire::{Capture, Hello, Record};
use super::{Capability, CapabilityState, ValidatedScope};
use crate::inspection::RuntimeIdentity;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub(super) type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

const EXPIRY: Duration = Duration::from_secs(30);
const FLOOR: Duration = Duration::from_millis(500);

pub(super) struct Session {
    inner: Arc<Mutex<Inner>>,
    budget: Budget,
    requests: Arc<tokio::sync::Semaphore>,
    _fixed: Charge,
    now: Clock,
}
impl Default for Session {
    fn default() -> Self {
        Self::with_clock(Arc::new(Instant::now))
    }
}
impl Session {
    pub fn with_clock(now: Clock) -> Self {
        let budget = Budget::default();
        let fixed = budget.reserve(8 * MIB).expect("initial identity budget");
        Self {
            inner: Arc::new(Mutex::new(Inner {
                now: now.clone(),
                identity: None,
                connection: None,
                latest: None,
                last_start: None,
                refusal: None,
                incarnation: None,
                sequence: 0,
                accept_incarnation_change: false,
                expired: false,
                failed_refresh: false,
            })),
            budget,
            requests: Arc::new(tokio::sync::Semaphore::new(8)),
            _fixed: fixed,
            now,
        }
    }
}

struct Inner {
    now: Clock,
    identity: Option<RuntimeIdentity>,
    connection: Option<Connection>,
    latest: Option<TimedCapture>,
    last_start: Option<Instant>,
    refusal: Option<&'static str>,
    incarnation: Option<String>,
    sequence: u64,
    accept_incarnation_change: bool,
    expired: bool,
    failed_refresh: bool,
}

struct TimedCapture {
    capture: Capture,
    hello: Hello,
    start: Instant,
    published: Instant,
}

#[derive(Serialize)]
pub(super) struct PageKey {
    pub volid: u16,
    pub pageid: u32,
}

#[derive(Serialize)]
struct Row<'a> {
    volid: u16,
    pageid: u32,
    state: &'static str,
    reason: &'static str,
    evidence: Option<&'a Record>,
}

#[derive(Serialize)]
struct CaptureMetadata<'a> {
    identity: String,
    incarnation_binding: String,
    incarnation: &'a str,
    protocol_major: u8,
    protocol_minor: u32,
    database_fingerprint: String,
    sequence: String,
    start_time_us: &'a str,
    end_time_us: &'a str,
    upper_age_ms: u64,
    shared_lru_count: u32,
    private_lru_count: u32,
}

#[derive(Serialize)]
struct Response<'a> {
    schema: &'static str,
    schema_version: u8,
    capability: Capability,
    pages: Vec<PageKey>,
    epoch: String,
    generation: &'a str,
    requested_count: usize,
    evaluated_count: usize,
    producer_complete: Option<bool>,
    capture: Option<CaptureMetadata<'a>>,
    observations: Vec<Row<'a>>,
    limitations: &'static [&'static str],
}

impl Session {
    pub fn capability(&self) -> Capability {
        let Ok(mut inner) = self.inner.try_lock() else {
            return capability(CapabilityState::Connecting, "observation-in-flight");
        };
        inner.expire();
        inner.capability()
    }

    pub async fn observe(
        &self,
        path: &Path,
        identity: Option<RuntimeIdentity>,
        scope: ValidatedScope,
        generation: &str,
        retry: bool,
    ) -> Result<ResponseBytes, &'static str> {
        let permit = self
            .requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| "runtime-admission-refused")?;
        let charge = self.budget.reserve(2 * MIB)?;
        let demanded = (self.now)();
        tokio::time::timeout(Duration::from_millis(2500), async {
            let mut inner = self.inner.lock().await;
            inner.expire();
            if identity.is_none()
                || inner
                    .identity
                    .as_ref()
                    .is_some_and(|previous| Some(previous) != identity.as_ref())
            {
                inner.latest = None;
                inner.connection = None;
                inner.refusal = Some("identity-mismatch");
            }
            inner.identity = identity;
            if retry && inner.identity.is_some() && inner.refusal.take().is_some() {
                inner.accept_incarnation_change = true;
            }
            if inner.refusal.is_none() {
                let shared = inner.latest.as_ref().is_some_and(|latest| {
                    latest.published >= demanded || inner.age(latest.start) < FLOOR
                });
                if !shared {
                    if let Some(start) = inner.last_start
                        && let Some(delay) = FLOOR.checked_sub(inner.age(start))
                    {
                        tokio::time::sleep(delay).await;
                    }
                    if inner
                        .latest
                        .as_ref()
                        .is_some_and(|latest| inner.age(latest.start) >= Duration::from_secs(27))
                    {
                        inner.latest = None;
                        inner.expired = true;
                    }
                    if let Err(reason) = inner.refresh(path, self.budget.clone()).await {
                        inner.failed_refresh = true;
                        if matches!(
                            reason,
                            "identity-mismatch"
                                | "peer-refused"
                                | "peer-unverifiable"
                                | "version-unsupported"
                                | "incarnation-changed"
                                | "identity-oversized"
                        ) {
                            inner.latest = None;
                            inner.refusal = Some(reason);
                        }
                    } else {
                        inner.failed_refresh = false;
                        let expires =
                            inner.latest.as_ref().expect("published capture").start + EXPIRY;
                        let weak = Arc::downgrade(&self.inner);
                        tokio::spawn(async move {
                            tokio::time::sleep_until(tokio::time::Instant::from_std(expires)).await;
                            if let Some(inner) = weak.upgrade() {
                                inner.lock().await.expire();
                            }
                        });
                    }
                }
            }
            inner.expire();
            let bytes = inner.serialize(&scope, generation)?;
            if (self.now)()
                .checked_duration_since(demanded)
                .unwrap_or(Duration::MAX)
                >= Duration::from_millis(2500)
            {
                return Err("runtime-deadline-exceeded");
            }
            Ok(ResponseBytes {
                bytes,
                _charge: charge,
                _permit: permit,
            })
        })
        .await
        .map_err(|_| "runtime-deadline-exceeded")?
    }
}

impl Inner {
    fn age(&self, start: Instant) -> Duration {
        (self.now)()
            .checked_duration_since(start)
            .unwrap_or(Duration::MAX)
    }
    fn expire(&mut self) {
        if self
            .latest
            .as_ref()
            .is_some_and(|latest| self.age(latest.start) >= EXPIRY)
        {
            self.latest = None;
            self.expired = true;
        }
    }

    fn capability(&self) -> Capability {
        if let Some(reason) = self.refusal {
            return capability(
                if reason == "version-unsupported" {
                    CapabilityState::Incompatible
                } else {
                    CapabilityState::Refused
                },
                reason,
            );
        }
        if self.latest.is_some() {
            capability(
                if self.failed_refresh {
                    CapabilityState::Stale
                } else {
                    CapabilityState::Active
                },
                "observation-available",
            )
        } else {
            capability(CapabilityState::Unavailable, "no-usable-observation")
        }
    }

    async fn refresh(&mut self, path: &Path, budget: Budget) -> Result<(), &'static str> {
        // Taking the connection makes cancellation close the unfinished stream.
        // Previously published evidence remains independently owned and aged.
        let mut connection = match self.connection.take() {
            Some(connection) => connection,
            None => Connection::with_budget(path, budget).await?,
        };
        let hello = connection.hello();
        if !matches_identity(hello, self.identity.as_ref().ok_or("identity-mismatch")?) {
            return Err("identity-mismatch");
        }
        if self
            .incarnation
            .as_ref()
            .is_some_and(|previous| previous != &hello.incarnation)
        {
            self.latest = None;
            if !self.accept_incarnation_change {
                return Err("incarnation-changed");
            }
            self.sequence = 0;
        }
        self.incarnation = Some(hello.incarnation.clone());
        self.accept_incarnation_change = false;
        let hello = hello.clone();
        let start = (self.now)();
        self.last_start = Some(start);
        connection.set_sequence_floor(self.sequence);
        let mut lease = ScanLease {
            connection: Some(connection),
            sequence: &mut self.sequence,
        };
        let result = lease
            .connection
            .as_mut()
            .expect("active scan connection")
            .scan()
            .await;
        let connection = lease.finish();
        let capture = result?;
        self.expired = false;
        self.latest = Some(TimedCapture {
            capture,
            hello,
            start,
            published: (self.now)(),
        });
        self.connection = Some(connection);
        Ok(())
    }

    fn serialize(&self, scope: &ValidatedScope, generation: &str) -> Result<Vec<u8>, &'static str> {
        let serialization_started = Instant::now();
        let latest = self.latest.as_ref();
        let mut rows = Vec::with_capacity(scope.pages().len());
        let mut pages = Vec::with_capacity(scope.pages().len());
        let mut evaluated = 0;
        for page in scope.pages() {
            let volid = u16::try_from(page.vol_id.get()).map_err(|_| "invalid-scope")?;
            let pageid = u32::try_from(page.page_id.get()).map_err(|_| "invalid-scope")?;
            pages.push(PageKey { volid, pageid });
            let in_scope = self.identity.as_ref().is_some_and(|identity| {
                identity.volumes.iter().any(|volume| volume.volid == volid)
            });
            if latest.is_some() && in_scope {
                evaluated += 1;
            }
            let (state, reason, evidence) = match latest {
                Some(_) if !in_scope => ("unknown", "unevaluated", None),
                None if self.expired => ("expired", "observation-expired", None),
                None => ("unavailable", "no-usable-observation", None),
                Some(latest) => match latest.capture.record(volid, pageid) {
                    Ok(Some(record)) => ("resident", "observed-resident", Some(record)),
                    Ok(None) if latest.capture.truncated => ("unknown", "partial-omission", None),
                    Ok(None) => ("not-resident", "observed-not-resident", None),
                    Err(()) => ("unknown", "duplicate-vpid", None),
                },
            };
            rows.push(Row {
                volid,
                pageid,
                state,
                reason,
                evidence,
            });
        }
        let metadata = latest.map(|latest| {
            let hello = &latest.hello;
            CaptureMetadata {
                identity: digest(
                    format!("{}:{}", hello.incarnation, latest.capture.sequence).as_bytes(),
                ),
                incarnation_binding: digest(hello.incarnation.as_bytes()),
                incarnation: &hello.incarnation[..12],
                protocol_major: 1,
                protocol_minor: hello.minor,
                database_fingerprint: digest(
                    format!("{}:{:?}", hello.database_creation, hello.volumes).as_bytes(),
                ),
                sequence: latest.capture.sequence.to_string(),
                start_time_us: &latest.capture.start_time_us,
                end_time_us: &latest.capture.end_time_us,
                upper_age_ms: u64::try_from(self.age(latest.start).as_millis())
                    .unwrap_or(u64::MAX)
                    .saturating_add(101),
                shared_lru_count: hello.shared,
                private_lru_count: hello.private,
            }
        });
        let response = Response {
            schema: "volmap.runtime.page-buffer",
            schema_version: 1,
            capability: self.capability(),
            pages,
            epoch: scope.epoch().to_string(),
            generation,
            requested_count: scope.pages().len(),
            evaluated_count: evaluated,
            producer_complete: latest.map(|latest| !latest.capture.truncated),
            capture: metadata,
            observations: rows,
            limitations: &[
                "Capture interval, not a page timestamp.",
                "Latch and LRU tuples are individually coherent; records and scans are not atomic.",
                "Observed residency is not current residency.",
                "No page-image correspondence, commit visibility, durability or event causality is established.",
            ],
        };
        let mut output = LimitedWriter(Vec::with_capacity(1024 * 1024));
        serde_json::to_writer(&mut output, &response).map_err(|_| "response-limit")?;
        if serialization_started.elapsed() > Duration::from_millis(100) {
            return Err("serialization-deadline");
        }
        Ok(output.0)
    }
}

// Even cancellation after a valid header consumes its incarnation-local
// sequence. The floor survives dropping an unfinished connection.
struct ScanLease<'a> {
    connection: Option<Connection>,
    sequence: &'a mut u64,
}
impl ScanLease<'_> {
    fn finish(mut self) -> Connection {
        let connection = self.connection.take().expect("active scan connection");
        *self.sequence = (*self.sequence).max(connection.sequence());
        connection
    }
}
impl Drop for ScanLease<'_> {
    fn drop(&mut self) {
        if let Some(connection) = &self.connection {
            *self.sequence = (*self.sequence).max(connection.sequence());
        }
    }
}

fn matches_identity(hello: &Hello, expected: &RuntimeIdentity) -> bool {
    hello.database_creation == expected.database_creation.to_string()
        && hello.volumes.len() == expected.volumes.len()
        && hello
            .volumes
            .iter()
            .zip(&expected.volumes)
            .all(|(actual, expected)| {
                actual.volid == expected.volid
                    && actual.volume_creation == expected.volume_creation.to_string()
                    && actual.device == expected.device.to_string()
                    && actual.inode == expected.inode.to_string()
            })
}

fn capability(state: CapabilityState, reason: &'static str) -> Capability {
    Capability {
        schema: "volmap.runtime",
        schema_version: 1,
        source: "cubrid-page-buffer-observation",
        state,
        verification: if matches!(state, CapabilityState::Active | CapabilityState::Stale) {
            "verified"
        } else {
            "unverified"
        },
        reason,
    }
}

fn digest(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(result, "{byte:02x}").expect("write into String");
    }
    result
}

struct LimitedWriter(Vec<u8>);
impl std::io::Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > (1024_usize * 1024).saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("response limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
