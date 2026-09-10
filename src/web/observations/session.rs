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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{Mutex, Notify, OwnedMutexGuard};

pub(super) type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

/// Wall time can revoke trust in elapsed time, but cannot renew capture age.
/// In particular Linux monotonic time may stop during system suspension.
pub(super) fn conservative_clock(
    read: Arc<dyn Fn() -> (Instant, SystemTime) + Send + Sync>,
) -> Clock {
    let first = read();
    let previous = std::sync::Mutex::new((first, first.0));
    Arc::new(move || {
        let mut previous = previous.lock().expect("observation clock");
        let current = read();
        let monotonic = current.0.checked_duration_since(previous.0.0);
        let wall = current.1.duration_since(previous.0.1).ok();
        let elapsed = match (monotonic, wall) {
            (Some(monotonic), Some(wall)) if monotonic.abs_diff(wall) <= Duration::from_secs(1) => {
                monotonic.max(wall)
            }
            _ => EXPIRY,
        };
        previous.1 += elapsed;
        previous.0 = current;
        previous.1
    })
}

const EXPIRY: Duration = Duration::from_secs(30);
const FLOOR: Duration = Duration::from_millis(500);

pub(super) struct Session {
    inner: Arc<Mutex<Inner>>,
    budget: Budget,
    requests: Arc<tokio::sync::Semaphore>,
    _fixed: Charge,
    now: Clock,
    observers: Arc<Observers>,
    sleep: super::Scheduler,
}
impl Session {
    pub(super) fn new(sleep: super::Scheduler) -> Self {
        let session = Self::with_scheduler(
            conservative_clock(Arc::new(|| (Instant::now(), SystemTime::now()))),
            sleep,
        );
        session.inner.try_lock().expect("new session").scan_now = Arc::new(Instant::now);
        session
    }

    pub(super) fn with_scheduler(now: Clock, sleep: super::Scheduler) -> Self {
        let mut session = Self::with_clock(now);
        session.sleep = sleep;
        session
    }

    pub fn with_clock(now: Clock) -> Self {
        let budget = Budget::default();
        let fixed = budget.reserve(8 * MIB).expect("initial identity budget");
        Self {
            inner: Arc::new(Mutex::new(Inner {
                now: now.clone(),
                scan_now: now.clone(),
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
                failure_reason: "no-usable-observation",
                revision: 0,
            })),
            budget,
            requests: Arc::new(tokio::sync::Semaphore::new(8)),
            _fixed: fixed,
            now,
            observers: Arc::new(Observers::default()),
            sleep: Arc::new(super::monotonic_sleep),
        }
    }
}

#[derive(Default)]
struct Observers {
    count: AtomicUsize,
    changed: Notify,
}
impl Observers {
    async fn empty(&self) {
        loop {
            if self.count.load(Ordering::SeqCst) == 0 {
                return;
            }
            self.changed.notified().await;
        }
    }
}
struct Demand(Arc<Observers>);
impl Drop for Demand {
    fn drop(&mut self) {
        self.0.count.fetch_sub(1, Ordering::SeqCst);
        self.0.changed.notify_one();
    }
}

struct Inner {
    now: Clock,
    scan_now: Clock,
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
    failure_reason: &'static str,
    revision: u64,
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
    #[cfg(test)]
    pub(super) fn retained_identity(&self) -> Option<RuntimeIdentity> {
        self.inner
            .try_lock()
            .ok()
            .and_then(|inner| inner.identity.clone())
    }

    pub async fn capability(
        &self,
        path: &Path,
        current_identity: Option<RuntimeIdentity>,
    ) -> Capability {
        let Ok(mut inner) = self.inner.try_lock() else {
            return capability(CapabilityState::Connecting, "observation-in-flight");
        };
        inner.expire();
        if inner.identity.is_some() && inner.identity != current_identity {
            inner.fail("identity-mismatch");
            inner.identity = current_identity;
        }
        if inner.identity.is_some() && inner.refusal.is_none() {
            match inner.connect(path, self.budget.clone()).await {
                Ok(connection) => inner.connection = Some(connection),
                Err(reason) => inner.fail(reason),
            }
        }
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
        self.observers.count.fetch_add(1, Ordering::SeqCst);
        let _demand = Demand(self.observers.clone());
        let demanded = (self.now)();
        tokio::time::timeout(Duration::from_millis(2500), async {
            let mut inner = self.inner.clone().lock_owned().await;
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
                inner.revision += 1;
            }
            inner.identity = identity;
            if retry && inner.identity.is_some() && inner.refusal.take().is_some() {
                inner.accept_incarnation_change = true;
            }
            if inner.refusal.is_none() {
                let shared = inner.latest.as_ref().is_some_and(|latest| {
                    if scope.after_request {
                        latest.start > demanded
                    } else {
                        latest.published >= demanded
                            || inner
                                .age(latest.start)
                                .saturating_add(Duration::from_millis(101))
                                <= scope.cadence
                    }
                });
                if !shared {
                    let observers = self.observers.clone();
                    let budget = self.budget.clone();
                    let path = path.to_path_buf();
                    let sleep = self.sleep.clone();
                    let owner = Arc::downgrade(&self.inner);
                    // Ownership is session-wide: dropping one HTTP future must
                    // not cancel another admitted caller's refresh demand.
                    inner = tokio::spawn(async move {
                        tokio::select! {
                            biased;
                            () = observers.empty() => {},
                            () = refresh_demand(&mut inner, &path, budget, sleep, owner) => {},
                        }
                        inner
                    })
                    .await
                    .map_err(|_| "runtime-refresh-failed")?;
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

async fn refresh_demand(
    inner: &mut OwnedMutexGuard<Inner>,
    path: &Path,
    budget: Budget,
    sleep: super::Scheduler,
    owner: std::sync::Weak<Mutex<Inner>>,
) {
    if let Some(start) = inner.last_start
        && let Some(delay) = FLOOR.checked_sub(
            (inner.scan_now)()
                .checked_duration_since(start)
                .unwrap_or_default(),
        )
    {
        (sleep)(delay).await;
    }
    if inner
        .latest
        .as_ref()
        .is_some_and(|latest| inner.age(latest.start) >= Duration::from_secs(27))
    {
        inner.latest = None;
        inner.expired = true;
    }
    if let Err(reason) = inner.refresh(path, budget).await {
        inner.fail(reason);
    } else {
        inner.failed_refresh = false;
        inner.revision += 1;
        let remaining = EXPIRY
            .saturating_sub(inner.age(inner.latest.as_ref().expect("published capture").start));
        let weak = owner;
        tokio::spawn(async move {
            tokio::time::sleep(remaining).await;
            if let Some(inner) = weak.upgrade() {
                inner.lock().await.expire();
            }
        });
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
            self.revision += 1;
        }
    }

    fn fail(&mut self, reason: &'static str) {
        self.failed_refresh = true;
        self.failure_reason = match reason {
            "producer-busy" => "busy",
            "rate-limited" => "rate-limited",
            "parameter-off" => "parameter-off",
            _ => "no-usable-observation",
        };
        self.revision += 1;
        if matches!(
            reason,
            "identity-mismatch"
                | "peer-refused"
                | "peer-unverifiable"
                | "version-unsupported"
                | "incarnation-changed"
                | "identity-oversized"
        ) {
            self.latest = None;
            self.connection = None;
            self.refusal = Some(reason);
        }
    }

    fn capability(&self) -> Capability {
        let mut result = if let Some(reason) = self.refusal {
            capability(
                if reason == "version-unsupported" {
                    CapabilityState::Incompatible
                } else {
                    CapabilityState::Refused
                },
                reason,
            )
        } else if self.latest.is_some() {
            capability(
                if self.failed_refresh {
                    CapabilityState::Stale
                } else {
                    CapabilityState::Active
                },
                if self.failed_refresh {
                    self.failure_reason
                } else {
                    "observation-available"
                },
            )
        } else {
            capability(
                CapabilityState::Unavailable,
                if self.expired {
                    "observation-expired"
                } else {
                    self.failure_reason
                },
            )
        };
        result.revision = self.revision.to_string();
        if self.refusal.is_none() {
            result.incarnation_binding = self
                .incarnation
                .as_ref()
                .map(|incarnation| digest(incarnation.as_bytes()));
            result.capture_identity = self.latest.as_ref().map(|latest| {
                digest(
                    format!("{}:{}", latest.hello.incarnation, latest.capture.sequence).as_bytes(),
                )
            });
        }
        result
    }

    async fn connect(&mut self, path: &Path, budget: Budget) -> Result<Connection, &'static str> {
        let mut existing = self.connection.take();
        if let Some(connection) = &existing
            && !connection.is_current(path)?
        {
            existing = None;
        }
        let connection = match existing {
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
        Ok(connection)
    }

    async fn refresh(&mut self, path: &Path, budget: Budget) -> Result<(), &'static str> {
        // Taking the connection makes cancellation close the unfinished stream.
        let mut connection = self.connect(path, budget).await?;
        let hello = connection.hello();
        let hello = hello.clone();
        let start = (self.now)();
        self.last_start = Some((self.scan_now)());
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
        incarnation_binding: None,
        capture_identity: None,
        revision: "0".into(),
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

#[cfg(test)]
mod response_tests {
    use super::LimitedWriter;
    use std::io::Write as _;

    #[test]
    fn normalized_response_cap_includes_all_serialized_bytes_without_growing_past_it() {
        for length in [1_048_575, 1_048_576, 1_048_577] {
            let mut output = LimitedWriter(Vec::with_capacity(1_048_576));
            let chunk = [b' '; 4096];
            let mut accepted = true;
            for offset in (0..length).step_by(chunk.len()) {
                if output
                    .write_all(&chunk[..chunk.len().min(length - offset)])
                    .is_err()
                {
                    accepted = false;
                    break;
                }
            }
            assert_eq!(accepted, length <= 1_048_576);
            assert!(output.0.len() <= 1_048_576);
            assert_eq!(output.0.capacity(), 1_048_576);
        }
    }
}
