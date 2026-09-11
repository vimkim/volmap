//! Private authenticated observation broker, independent of disk-follow ownership.
#[cfg(test)]
mod lifecycle_tests;
mod memory;
mod session;
mod socket;
mod wire;

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::model::Vpid;
use serde::{Deserialize, Serialize};

/// Structural scope validation only: VPID types enforce nonnegative IDs and
/// the bound is checked before copying. Identity/membership proof belongs to
/// verified attachment. Ordering is retained.
#[derive(Debug)]
pub(super) struct ValidatedScope {
    pages: Box<[Vpid]>,
    epoch: u64,
    cadence: Duration,
    after_request: bool,
    sector: Option<SectorScope>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum SectorScope {
    Sector { volid: i16, sectorid: i32 },
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ScopeError {
    TooManyPages,
    InvalidCadence,
    InvalidAddressing,
}

impl ValidatedScope {
    pub(super) fn new(pages: &[Vpid], epoch: u64) -> Result<Self, ScopeError> {
        if pages.len() > 512 {
            return Err(ScopeError::TooManyPages);
        }
        Ok(Self {
            pages: pages.into(),
            epoch,
            cadence: Duration::from_millis(500),
            after_request: false,
            sector: None,
        })
    }

    pub(super) fn for_sector(
        sector: SectorScope,
        view: &crate::inspection::GraphView,
        epoch: u64,
    ) -> Result<Self, ScopeError> {
        let SectorScope::Sector { volid, sectorid } = sector;
        let volid = crate::model::VolId::new(volid).map_err(|_| ScopeError::InvalidAddressing)?;
        let sectorid =
            crate::model::SectorId::new(sectorid).map_err(|_| ScopeError::InvalidAddressing)?;
        let projection = view
            .sector(volid, sectorid)
            .map_err(|_| ScopeError::InvalidAddressing)?;
        let pages: Vec<_> = projection.pages.iter().map(|page| page.vpid).collect();
        let mut scope = Self::new(&pages, epoch)?;
        scope.sector = Some(sector);
        Ok(scope)
    }

    pub(super) fn with_demand(
        mut self,
        cadence_ms: u64,
        after_request: bool,
    ) -> Result<Self, ScopeError> {
        if !(100..=30_000).contains(&cadence_ms) {
            return Err(ScopeError::InvalidCadence);
        }
        self.cadence = Duration::from_millis(cadence_ms);
        self.after_request = after_request;
        Ok(self)
    }

    pub(super) fn pages(&self) -> &[Vpid] {
        &self.pages
    }

    pub(super) fn epoch(&self) -> u64 {
        self.epoch
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CapabilityState {
    Disabled,
    Connecting,
    Active,
    Stale,
    Unavailable,
    Refused,
    Incompatible,
}

#[derive(Serialize)]
pub(super) struct Capability {
    schema: &'static str,
    schema_version: u8,
    source: &'static str,
    state: CapabilityState,
    verification: &'static str,
    reason: &'static str,
    incarnation_binding: Option<String>,
    capture_identity: Option<String>,
    revision: String,
}

pub(super) struct Broker {
    adapter: Adapter,
    sleep: Scheduler,
    session: session::Session,
}

pub(super) type SleepFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
pub(super) type Scheduler = Arc<dyn Fn(Duration) -> SleepFuture + Send + Sync>;

fn monotonic_sleep(duration: Duration) -> SleepFuture {
    Box::pin(tokio::time::sleep(duration))
}

enum Adapter {
    Disabled,
    Socket {
        path: PathBuf,
    },
    #[cfg(test)]
    Simulated {
        arrived: std::sync::mpsc::Sender<()>,
        release: std::sync::Arc<tokio::sync::Semaphore>,
    },
}

impl Broker {
    pub(super) fn new(socket: Option<PathBuf>) -> Self {
        Self::with_scheduler(socket, Arc::new(monotonic_sleep))
    }

    pub(super) fn with_scheduler(socket: Option<PathBuf>, sleep: Scheduler) -> Self {
        Self {
            adapter: socket.map_or(Adapter::Disabled, |path| Adapter::Socket { path }),
            session: session::Session::new(sleep.clone()),
            sleep,
        }
    }

    #[cfg(test)]
    pub(super) fn simulated(
        arrived: std::sync::mpsc::Sender<()>,
        release: std::sync::Arc<tokio::sync::Semaphore>,
    ) -> Self {
        Self::simulated_with_scheduler(arrived, release, Arc::new(monotonic_sleep))
    }

    #[cfg(test)]
    pub(super) fn simulated_with_scheduler(
        arrived: std::sync::mpsc::Sender<()>,
        release: Arc<tokio::sync::Semaphore>,
        sleep: Scheduler,
    ) -> Self {
        Self {
            adapter: Adapter::Simulated { arrived, release },
            session: session::Session::new(sleep.clone()),
            sleep,
        }
    }

    #[cfg(test)]
    fn with_clock(socket: PathBuf, now: session::Clock) -> Self {
        let mut broker = Self::new(Some(socket));
        broker.session = session::Session::with_clock(now);
        broker
    }

    pub(super) async fn observe(
        &self,
        scope: ValidatedScope,
        identity: Option<crate::inspection::RuntimeIdentity>,
        generation: &str,
        retry: bool,
    ) -> Result<memory::ResponseBytes, &'static str> {
        match &self.adapter {
            Adapter::Socket { path } => {
                self.session
                    .observe(path, identity, scope, generation, retry)
                    .await
            }
            Adapter::Disabled => Err("runtime-disabled"),
            #[cfg(test)]
            Adapter::Simulated { .. } => Err("runtime-disabled"),
        }
    }

    #[cfg(test)]
    pub(super) async fn capabilities(&self) -> Result<Capability, ()> {
        self.capabilities_for(self.session.retained_identity())
            .await
    }

    pub(super) async fn capabilities_for(
        &self,
        identity: Option<crate::inspection::RuntimeIdentity>,
    ) -> Result<Capability, ()> {
        tokio::select! {
            biased;
            () = (self.sleep)(Duration::from_secs(1)) => Err(()),
            capability = self.read_capability(identity) => Ok(capability),
        }
    }

    fn read_capability(
        &self,
        identity: Option<crate::inspection::RuntimeIdentity>,
    ) -> Pin<Box<dyn Future<Output = Capability> + Send + '_>> {
        Box::pin(async move {
            let (state, reason) = match &self.adapter {
                Adapter::Disabled => (CapabilityState::Disabled, "not-requested"),
                Adapter::Socket { path } => return self.session.capability(path, identity).await,
                #[cfg(test)]
                Adapter::Simulated { arrived, release } => {
                    let _ = arrived.send(());
                    release
                        .acquire()
                        .await
                        .expect("fixture release gate")
                        .forget();
                    (CapabilityState::Unavailable, "attachment-not-implemented")
                }
            };
            Capability {
                schema: "volmap.runtime",
                schema_version: 1,
                source: "cubrid-page-buffer-observation",
                state,
                verification: "unverified",
                reason,
                incarnation_binding: None,
                capture_identity: None,
                revision: "0".into(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PageId, VolId, Vpid};

    #[test]
    fn observation_scope_enforces_the_bound_before_copying_and_keeps_order() {
        let page = Vpid::new(VolId::new(0).unwrap(), PageId::new(3).unwrap());
        let pages = [page; 513];
        for count in [0, 1, 511, 512] {
            let scope = ValidatedScope::new(&pages[..count], u64::MAX).unwrap();
            assert_eq!(scope.pages(), &pages[..count]);
            assert_eq!(scope.epoch(), u64::MAX);
        }
        assert!(matches!(
            ValidatedScope::new(&pages, 1),
            Err(ScopeError::TooManyPages)
        ));
        let mut original = [page];
        let scope = ValidatedScope::new(&original, 0).unwrap();
        original[0] = Vpid::new(VolId::new(1).unwrap(), PageId::new(9).unwrap());
        assert_ne!(scope.pages(), &original);
        assert_eq!(scope.pages(), &[page]);
    }

    #[derive(Clone, Default)]
    struct ManualScheduler(std::sync::Arc<std::sync::Mutex<ManualTime>>);

    #[derive(Default)]
    struct ManualTime {
        now: Duration,
        deadlines: Vec<(Duration, tokio::sync::oneshot::Sender<()>)>,
    }

    impl ManualScheduler {
        fn sleep(&self, delay: Duration) -> SleepFuture {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            let mut time = self.0.lock().unwrap();
            let deadline = time.now + delay;
            time.deadlines.push((deadline, sender));
            Box::pin(async move {
                let _ = receiver.await;
            })
        }

        fn advance(&self, delay: Duration) {
            let mut time = self.0.lock().unwrap();
            time.now += delay;
            let now = time.now;
            let deadlines = std::mem::take(&mut time.deadlines);
            for (deadline, sender) in deadlines {
                if deadline <= now {
                    let _ = sender.send(());
                } else {
                    time.deadlines.push((deadline, sender));
                }
            }
        }

        fn scheduler(&self) -> Scheduler {
            let clock = self.clone();
            std::sync::Arc::new(move |delay| clock.sleep(delay))
        }
    }

    #[test]
    fn injected_scheduler_controls_capability_deadline_boundaries() {
        use std::task::{Context, Poll, Waker};
        for milliseconds in [999, 1000, 1001] {
            let clock = ManualScheduler::default();
            let (arrived, arrivals) = std::sync::mpsc::channel();
            let release = Arc::new(tokio::sync::Semaphore::new(0));
            let broker =
                Broker::simulated_with_scheduler(arrived, release.clone(), clock.scheduler());
            let mut request = std::pin::pin!(broker.capabilities());
            let mut context = Context::from_waker(Waker::noop());
            assert!(request.as_mut().poll(&mut context).is_pending());
            arrivals.try_recv().unwrap();
            clock.advance(Duration::from_millis(milliseconds));
            if milliseconds < 1000 {
                assert!(request.as_mut().poll(&mut context).is_pending());
            }
            release.add_permits(1);
            match request.as_mut().poll(&mut context) {
                Poll::Ready(Ok(capability)) if milliseconds < 1000 => assert_eq!(
                    serde_json::to_value(capability).unwrap()["state"],
                    "unavailable"
                ),
                Poll::Ready(Err(())) if milliseconds >= 1000 => {}
                _ => panic!("incorrect capability deadline result at {milliseconds} ms"),
            }
        }
    }

    #[test]
    fn configured_and_disabled_brokers_accept_an_injected_scheduler() {
        use std::task::{Context, Poll, Waker};
        for socket in [None, Some(PathBuf::from("/private/not-connected.sock"))] {
            let expired: Scheduler = Arc::new(|_| Box::pin(std::future::ready(())));
            let broker = Broker::with_scheduler(socket, expired);
            let mut request = std::pin::pin!(broker.capabilities());
            assert!(matches!(
                request
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop())),
                Poll::Ready(Err(()))
            ));
        }
    }
}

#[cfg(test)]
mod wire_tests {
    use super::wire::Decoder;

    #[test]
    fn producer_duplicate_members_and_excessive_depth_are_rejected() {
        for bytes in [
            b"{\"type\":\"server_hello\",\"type\":\"server_hello\"}\n".as_slice(),
            b"{\"type\":\"server_hello\",\"extra\":[[[[[[[[[[[[[[[[0]]]]]]]]]]]]]]]]}\n".as_slice(),
        ] {
            assert!(Decoder::default().feed(bytes).is_err());
        }
        let transcript = include_str!(
            "../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
        );
        let hello = transcript
            .lines()
            .find(|line| line.contains("server_hello"))
            .unwrap();
        let duplicate = hello.replace(
            "\"protocol_major\":1",
            "\"protocol_major\":1,\"protocol_major\":1",
        );
        assert!(
            Decoder::default()
                .feed(format!("{duplicate}\n").as_bytes())
                .is_err()
        );
        for depth in [15, 16, 17] {
            let nested = format!(
                "{},\"extra\":{}0{}}}\n",
                &hello[..hello.len() - 1],
                "[".repeat(depth - 1),
                "]".repeat(depth - 1)
            );
            assert_eq!(
                Decoder::default().feed(nested.as_bytes()).is_ok(),
                depth <= 16
            );
        }
    }

    fn assert_absent_state_omitted(
        path: &std::path::Path,
        published: Option<&super::wire::Capture>,
    ) {
        if path.ends_with("optional-state-absent") {
            let record = published.unwrap().record(0, 7).unwrap().unwrap();
            assert_eq!(
                serde_json::to_value(record).unwrap(),
                serde_json::json!({"volid":0,"pageid":7})
            );
        }
    }

    #[test]
    fn canonical_exchange_corpus_validates_all_server_stream_outcomes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/pgbuf-inspector/v1/corpus/exchanges");
        let mut cases = 0;
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if !path.is_dir() {
                continue;
            }
            let expected: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path.join("expected.json")).unwrap())
                    .unwrap();
            let bytes = std::fs::read(path.join("stream.jsonl")).unwrap();
            let mut decoder = Decoder::default();
            let mut published = None;
            let mut valid = true;
            let mut expected_incarnation = None;
            for line in bytes.split_inclusive(|byte| *byte == b'\n') {
                let value = serde_json::from_slice::<serde_json::Value>(line).unwrap_or_default();
                // Chronological transcripts include requests. The live adapter
                // emits these itself; exercise its ordering guard separately
                // from the received byte stream.
                if value["type"] == "client_hello" {
                    if super::wire::parse(line).is_err()
                        || !value["supported_majors"]
                            .as_array()
                            .is_some_and(|majors| majors.contains(&serde_json::json!(1)))
                    {
                        valid = false;
                        break;
                    }
                    expected_incarnation =
                        value["expected_incarnation"].as_str().map(str::to_owned);
                    continue;
                }
                if value["type"] == "server_hello"
                    && expected_incarnation
                        .as_ref()
                        .is_some_and(|expected| value["incarnation"] != *expected)
                {
                    valid = false;
                    break;
                }
                if value["type"] == "scan_request" {
                    if decoder.begin_scan().is_err() {
                        valid = false;
                        break;
                    }
                    continue;
                }
                for chunk in line.chunks(13) {
                    if let Err(reason) = decoder.feed(chunk) {
                        if value["type"] != "error"
                            || !matches!(
                                reason,
                                "producer-busy"
                                    | "identity-oversized"
                                    | "incarnation-changed"
                                    | "version-unsupported"
                                    | "rate-limited"
                            )
                        {
                            valid = false;
                        }
                        break;
                    }
                }
                if !valid {
                    break;
                }
                if let Some(capture) = decoder.take_capture() {
                    published = Some(capture);
                }
            }
            assert_absent_state_omitted(&path, published.as_ref());
            valid &= decoder.idle();
            assert_eq!(
                valid,
                expected["valid"].as_bool().unwrap(),
                "{}",
                path.display()
            );
            if valid {
                assert_eq!(
                    published.is_some(),
                    expected["published"].as_bool().unwrap(),
                    "{}",
                    path.display()
                );
                for lookup in expected["lookups"].as_array().unwrap() {
                    let volume_id = u16::try_from(lookup["volid"].as_u64().unwrap()).unwrap();
                    let pageid = u32::try_from(lookup["pageid"].as_u64().unwrap()).unwrap();
                    let capture = published.as_ref().unwrap();
                    let result = if capture.record(volume_id, pageid).is_err() {
                        "ambiguous"
                    } else {
                        capture.lookup(volume_id, pageid, lookup["evaluated"].as_bool().unwrap())
                    };
                    assert_eq!(result, lookup["result"], "{}", path.display());
                }
            }
            cases += 1;
        }
        assert_eq!(cases, 39);
    }

    #[test]
    fn additive_unknown_numbers_do_not_require_machine_numeric_range() {
        let transcript = include_str!(
            "../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
        );
        let hello = transcript.lines().nth(1).unwrap();
        let frame = format!(
            "{},\"future\":{{\"offset\":1e400}}}}\n",
            &hello[..hello.len() - 1]
        );
        assert!(Decoder::default().feed(frame.as_bytes()).is_ok());
    }

    #[test]
    fn wire_frame_limits_include_newline_at_below_and_above_boundaries() {
        let transcript = include_str!(
            "../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
        );
        let hello = transcript.lines().nth(1).unwrap();
        let header = transcript.lines().nth(3).unwrap();
        for (limit, handshake) in [(65_536, true), (4096, false)] {
            for length in [limit - 1, limit, limit + 1] {
                let mut decoder = Decoder::default();
                if !handshake {
                    decoder.feed(format!("{hello}\n").as_bytes()).unwrap();
                    decoder.begin_scan().unwrap();
                }
                let mut frame = if handshake {
                    hello.as_bytes().to_vec()
                } else {
                    header.as_bytes().to_vec()
                };
                frame.resize(length - 1, b' ');
                frame.push(b'\n');
                assert_eq!(
                    decoder.feed(&frame).is_ok(),
                    length <= limit,
                    "frame length {length}"
                );
            }
        }
    }

    #[test]
    fn raw_record_slot_and_framed_scan_limits_are_checked_before_deduplication() {
        let transcript = include_str!(
            "../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
        );
        let hello = format!("{}\n", transcript.lines().nth(1).unwrap());
        let header = format!("{}\n", transcript.lines().nth(3).unwrap());
        let page = b"{\"type\":\"page\",\"incarnation\":\"0123456789abcdef0123456789abcdef\",\"scan_seq\":\"1\",\"volid\":0,\"pageid\":7}\n";
        let footer = |count, slots| {
            format!(
                "{{\"type\":\"scan_footer\",\"incarnation\":\"0123456789abcdef0123456789abcdef\",\"scan_seq\":\"1\",\"end_time_us\":\"2\",\"record_count\":{count},\"visited_slots\":{slots},\"truncated\":false}}\n"
            )
        };
        let decoder = || {
            let mut decoder = Decoder::default();
            decoder.feed(hello.as_bytes()).unwrap();
            decoder.begin_scan().unwrap();
            decoder.feed(header.as_bytes()).unwrap();
            decoder
        };
        for count in [65_535, 65_536, 65_537] {
            let mut capture = decoder();
            let mut accepted = true;
            for _ in 0..count {
                if capture.feed(page).is_err() {
                    accepted = false;
                    break;
                }
            }
            assert_eq!(accepted, count <= 65_536);
            if accepted {
                capture.feed(footer(count, count).as_bytes()).unwrap();
                assert!(capture.capture().unwrap().record(0, 7).is_err());
            }
            let mut slots = decoder();
            assert_eq!(
                slots.feed(footer(0, count).as_bytes()).is_ok(),
                count <= 65_536
            );
        }
        for total in [67_108_863, 67_108_864, 67_108_865] {
            let mut scan = decoder();
            let end = footer(0, 0);
            let mut remaining = total - header.len() - end.len();
            let minimum = b"{\"type\":\"future\"}\n".len();
            while remaining > 0 {
                let length = if remaining > 4096 {
                    4096.min(remaining - minimum)
                } else {
                    remaining
                };
                let mut frame = b"{\"type\":\"future\"}".to_vec();
                frame.resize(length - 1, b' ');
                frame.push(b'\n');
                scan.feed(&frame).unwrap();
                remaining -= length;
            }
            assert_eq!(
                scan.feed(end.as_bytes()).is_ok(),
                total <= 67_108_864,
                "framed bytes {total}"
            );
        }
    }

    #[test]
    fn broker_refuses_each_identity_component_and_incomplete_volume_sets_before_scanning() {
        use crate::inspection::{RuntimeIdentity, RuntimeVolumeIdentity};
        use std::io::{BufRead as _, Read as _, Write as _};
        use std::os::unix::{fs::PermissionsExt as _, net::UnixListener};
        for mismatch in 0..6 {
            let directory = std::env::temp_dir()
                .join(format!("volmap-identity-{}-{mismatch}", std::process::id()));
            std::fs::create_dir(&directory).unwrap();
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
            let path = directory.join("producer.sock");
            let listener = UnixListener::bind(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            let producer = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                reader.read_line(&mut request).unwrap();
                let transcript = include_str!(
                    "../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
                );
                writeln!(stream, "{}", transcript.lines().nth(1).unwrap()).unwrap();
                let mut rest = Vec::new();
                reader.read_to_end(&mut rest).unwrap();
                assert!(
                    rest.is_empty(),
                    "unverified identity must not receive a scan request"
                );
            });
            let broker = super::Broker::new(Some(path));
            let mut identity = RuntimeIdentity {
                database_creation: 1,
                volumes: vec![RuntimeVolumeIdentity {
                    volid: 0,
                    volume_creation: 2,
                    device: 3,
                    inode: u64::MAX,
                }],
            };
            match mismatch {
                0 => identity.database_creation += 1,
                1 => identity.volumes[0].volume_creation += 1,
                2 => identity.volumes[0].device += 1,
                3 => identity.volumes[0].inode -= 1,
                4 => identity.volumes.clear(),
                _ => identity.volumes.push(RuntimeVolumeIdentity {
                    volid: 1,
                    volume_creation: 2,
                    device: 3,
                    inode: 4,
                }),
            }
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let response = broker
                        .observe(
                            super::ValidatedScope::new(&[], 1).unwrap(),
                            Some(identity),
                            "1",
                            false,
                        )
                        .await
                        .unwrap();
                    let value: serde_json::Value =
                        serde_json::from_slice(response.as_ref()).unwrap();
                    assert_eq!(value["capability"]["state"], "refused");
                    assert_eq!(value["capability"]["reason"], "identity-mismatch");
                    assert!(value["capture"].is_null());
                });
            producer.join().unwrap();
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn actual_io_deadlines_bound_stalled_handshake_and_scan() {
        use std::io::{BufRead as _, Read as _, Write as _};
        use std::os::unix::{fs::PermissionsExt as _, net::UnixListener};
        use std::time::{Duration, Instant};
        for handshake in [true, false] {
            let directory = std::env::temp_dir()
                .join(format!("volmap-timeout-{}-{handshake}", std::process::id()));
            std::fs::create_dir(&directory).unwrap();
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
            let path = directory.join("producer.sock");
            let listener = UnixListener::bind(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            let producer = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                reader.read_line(&mut request).unwrap();
                if !handshake {
                    let transcript = include_str!(
                        "../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
                    );
                    writeln!(stream, "{}", transcript.lines().nth(1).unwrap()).unwrap();
                }
                reader.read_to_end(&mut Vec::new()).unwrap();
            });
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    if handshake {
                        let start = Instant::now();
                        assert!(matches!(
                            super::socket::Connection::connect(&path).await,
                            Err("attachment-timeout")
                        ));
                        assert!(
                            (Duration::from_millis(450)..Duration::from_millis(750))
                                .contains(&start.elapsed())
                        );
                    } else {
                        let mut connection =
                            super::socket::Connection::connect(&path).await.unwrap();
                        let start = Instant::now();
                        assert!(matches!(connection.scan().await, Err("scan-timeout")));
                        assert!(
                            (Duration::from_millis(1900)..Duration::from_millis(2250))
                                .contains(&start.elapsed())
                        );
                    }
                });
            producer.join().unwrap();
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "One complete socket, admission, retention and expiry scenario"
    )]
    fn broker_shares_capture_charges_held_responses_retains_original_age_and_expires() {
        use crate::inspection::{RuntimeIdentity, RuntimeVolumeIdentity};
        use crate::model::{PageId, VolId, Vpid};
        use std::io::{BufRead as _, Write as _};
        use std::os::unix::{fs::PermissionsExt as _, net::UnixListener};
        use std::sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        };
        use std::time::{Duration, Instant};
        let directory = std::env::temp_dir().join(format!("volmap-broker-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("producer.sock");
        let listener = UnixListener::bind(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let producer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            let transcript = include_str!(
                "../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
            );
            writeln!(stream, "{}", transcript.lines().nth(1).unwrap()).unwrap();
            for sequence in 1..=2 {
                request.clear();
                reader.read_line(&mut request).unwrap();
                assert!(!request.is_empty());
                for line in transcript.lines().skip(3) {
                    if sequence == 2 && line.contains("scan_footer") {
                        continue;
                    }
                    writeln!(
                        stream,
                        "{}",
                        line.replace(
                            "\"scan_seq\":\"1\"",
                            &format!("\"scan_seq\":\"{sequence}\"")
                        )
                    )
                    .unwrap();
                }
            }
            drop(reader);
            drop(stream);
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            request.clear();
            reader.read_line(&mut request).unwrap();
            writeln!(stream, "{}", transcript.lines().nth(1).unwrap()).unwrap();
            request.clear();
            reader.read_line(&mut request).unwrap();
            // Reusing the header of an unfinished scan must fail after reconnect.
            for line in transcript.lines().skip(3) {
                if writeln!(
                    stream,
                    "{}",
                    line.replace("\"scan_seq\":\"1\"", "\"scan_seq\":\"2\"")
                )
                .is_err()
                {
                    break;
                }
            }
        });
        let ticks = Arc::new(AtomicU64::new(0));
        let origin = Instant::now();
        let clock = ticks.clone();
        let broker = super::Broker::with_clock(
            path,
            Arc::new(move || origin + Duration::from_millis(clock.load(Ordering::SeqCst))),
        );
        let identity = RuntimeIdentity {
            database_creation: 1,
            volumes: vec![RuntimeVolumeIdentity {
                volid: 0,
                volume_creation: 2,
                device: 3,
                inode: u64::MAX,
            }],
        };
        let page = Vpid::new(VolId::new(0).unwrap(), PageId::new(7).unwrap());
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut held = Vec::new();
                for epoch in 0..8 {
                    let response = broker
                        .observe(
                            super::ValidatedScope::new(&[page], epoch).unwrap(),
                            Some(identity.clone()),
                            "1",
                            false,
                        )
                        .await
                        .unwrap();
                    let value: serde_json::Value =
                        serde_json::from_slice(response.as_ref()).unwrap();
                    assert_eq!(value["observations"][0]["state"], "resident");
                    assert_eq!(value["capture"]["sequence"], "1");
                    held.push(response);
                }
                assert!(matches!(
                    broker
                        .observe(
                            super::ValidatedScope::new(&[page], 8).unwrap(),
                            Some(identity.clone()),
                            "1",
                            false
                        )
                        .await,
                    Err("runtime-admission-refused")
                ));
                held.pop();
                ticks.store(1000, Ordering::SeqCst);
                let response = broker
                    .observe(
                        super::ValidatedScope::new(&[page], 9).unwrap(),
                        Some(identity.clone()),
                        "1",
                        false,
                    )
                    .await
                    .unwrap();
                let value: serde_json::Value = serde_json::from_slice(response.as_ref()).unwrap();
                assert_eq!(value["capture"]["sequence"], "1");
                assert_eq!(value["capture"]["upper_age_ms"], 1101);
                assert_eq!(value["capability"]["state"], "stale");
                drop(response);
                ticks.store(2000, Ordering::SeqCst);
                let response = broker
                    .observe(
                        super::ValidatedScope::new(&[page], 10).unwrap(),
                        Some(identity.clone()),
                        "1",
                        false,
                    )
                    .await
                    .unwrap();
                let value: serde_json::Value = serde_json::from_slice(response.as_ref()).unwrap();
                assert_eq!(value["capture"]["sequence"], "1");
                assert_eq!(value["capture"]["upper_age_ms"], 2101);
                assert_eq!(value["capability"]["state"], "stale");
                ticks.store(30_000, Ordering::SeqCst);
                assert_eq!(
                    serde_json::to_value(broker.capabilities().await.unwrap()).unwrap()["state"],
                    "unavailable"
                );
            });
        producer.join().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "One gated scan proves all eight independent caller scopes and admission"
    )]
    fn eight_inflight_callers_share_one_slow_capture_and_reject_a_ninth() {
        use crate::inspection::{RuntimeIdentity, RuntimeVolumeIdentity};
        use crate::model::{PageId, VolId, Vpid};
        use std::io::{BufRead as _, Write as _};
        use std::os::unix::{fs::PermissionsExt as _, net::UnixListener};
        use std::sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        };
        use std::time::{Duration, Instant};
        let directory =
            std::env::temp_dir().join(format!("volmap-coalesce-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("producer.sock");
        let listener = UnixListener::bind(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let ticks = Arc::new(AtomicU64::new(0));
        let producer_clock = ticks.clone();
        let (arrived, arrival) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        let producer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            let transcript = include_str!(
                "../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
            );
            writeln!(stream, "{}", transcript.lines().nth(1).unwrap()).unwrap();
            request.clear();
            reader.read_line(&mut request).unwrap();
            arrived.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(3)).unwrap();
            producer_clock.store(600, Ordering::SeqCst);
            for line in transcript.lines().skip(3) {
                writeln!(stream, "{line}").unwrap();
            }
        });
        let origin = Instant::now();
        let broker = Arc::new(super::Broker::with_clock(
            path,
            Arc::new(move || origin + Duration::from_millis(ticks.load(Ordering::SeqCst))),
        ));
        let identity = RuntimeIdentity {
            database_creation: 1,
            volumes: vec![RuntimeVolumeIdentity {
                volid: 0,
                volume_creation: 2,
                device: 3,
                inode: u64::MAX,
            }],
        };
        let page = Vpid::new(VolId::new(0).unwrap(), PageId::new(7).unwrap());
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut tasks = Vec::new();
                for epoch in 0..8 {
                    let broker = broker.clone();
                    let identity = identity.clone();
                    tasks.push(tokio::spawn(async move {
                        broker
                            .observe(
                                super::ValidatedScope::new(
                                    &[
                                        Vpid::new(
                                            VolId::new(0).unwrap(),
                                            PageId::new(i32::try_from(epoch).unwrap()).unwrap(),
                                        ),
                                        page,
                                        Vpid::new(VolId::new(1).unwrap(), PageId::new(0).unwrap()),
                                    ],
                                    epoch,
                                )
                                .unwrap()
                                .with_demand(if epoch % 2 == 0 { 500 } else { 2000 }, false)
                                .unwrap(),
                                Some(identity),
                                "1",
                                false,
                            )
                            .await
                    }));
                }
                arrival.await.unwrap();
                // All spawned callers are polled before this task resumes.
                tokio::task::yield_now().await;
                assert!(matches!(
                    broker
                        .observe(
                            super::ValidatedScope::new(&[page], 9).unwrap(),
                            Some(identity),
                            "1",
                            false
                        )
                        .await,
                    Err("runtime-admission-refused")
                ));
                release.send(()).unwrap();
                let mut held = Vec::new();
                for (epoch, task) in tasks.into_iter().enumerate() {
                    let response = task.await.unwrap().unwrap();
                    let value: serde_json::Value =
                        serde_json::from_slice(response.as_ref()).unwrap();
                    assert_eq!(value["capture"]["sequence"], "1");
                    assert_eq!(value["capture"]["upper_age_ms"], 701);
                    assert_eq!(value["capability"]["state"], "active");
                    assert_eq!(value["epoch"], epoch.to_string());
                    assert_eq!(value["pages"][0]["pageid"], epoch);
                    assert_eq!(value["requested_count"], 3);
                    assert_eq!(value["evaluated_count"], 2);
                    assert_eq!(value["producer_complete"], true);
                    assert_eq!(value["observations"][1]["state"], "resident");
                    assert_eq!(value["observations"][2]["reason"], "unevaluated");
                    held.push(response);
                }
            });
        producer.join().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn peer_credentials_reject_another_effective_uid_without_a_root_exception() {
        use std::io::{BufRead as _, Read as _};
        use std::os::unix::fs::PermissionsExt as _;
        if std::env::var_os("VOLMAP_PEER_NAMESPACE").is_none() {
            let result = std::process::Command::new("unshare")
                .args(["--map-auto", "--map-root-user"])
                .arg(std::env::current_exe().unwrap())
                .args(["--exact", "web::observations::wire_tests::peer_credentials_reject_another_effective_uid_without_a_root_exception", "--nocapture"])
                .env("VOLMAP_PEER_NAMESPACE", "1").output().unwrap();
            assert!(
                result.status.success(),
                "credential isolation is required: {} {}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            return;
        }
        let directory = std::env::temp_dir().join(format!("volmap-peer-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o777)).unwrap();
        let path = directory.join("peer.sock");
        let mut producer = std::process::Command::new("python3")
            .args([
                "-u",
                "-c",
                r"
import os, socket, sys
os.setgid(1)
os.setuid(1)
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.bind(sys.argv[1])
s.listen(1)
print('ready', flush=True)
c, _ = s.accept()
assert c.recv(1) == b'', 'consumer trusted protocol before peer authentication'
print('no protocol accepted', flush=True)
",
            ])
            .arg(&path)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = std::io::BufReader::new(producer.stdout.take().unwrap());
        let mut ready = String::new();
        output.read_line(&mut ready).unwrap();
        assert_eq!(ready.trim(), "ready");
        assert!(
            std::process::Command::new("chown")
                .arg("0:0")
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                assert!(matches!(
                    super::socket::Connection::connect(&path).await,
                    Err("peer-refused")
                ));
            });
        let mut rest = String::new();
        output.read_to_string(&mut rest).unwrap();
        assert!(producer.wait().unwrap().success());
        assert!(rest.contains("no protocol accepted"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn actual_private_socket_accepts_chunked_producer_and_rejects_unsafe_mode() {
        use std::io::{BufRead, Write};
        use std::os::unix::{fs::PermissionsExt, net::UnixListener};
        let directory =
            std::env::temp_dir().join(format!("volmap-observation-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("producer.sock");
        let listener = UnixListener::bind(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let producer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let transcript = include_str!(
                "../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
            );
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            for line in transcript.lines() {
                if line.contains("client_hello") || line.contains("scan_request") {
                    let mut request = String::new();
                    reader.read_line(&mut request).unwrap();
                    assert!(!request.is_empty());
                } else {
                    for chunk in format!("{line}\n").as_bytes().chunks(7) {
                        stream.write_all(chunk).unwrap();
                    }
                }
            }
        });
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let mut connection = super::socket::Connection::connect(&path).await.unwrap();
                assert_eq!(
                    connection.scan().await.unwrap().lookup(0, 7, true),
                    "resident"
                );
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();
                assert!(matches!(
                    super::socket::Connection::connect(&path).await,
                    Err("peer-refused")
                ));
            });
        producer.join().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn complete_producer_scan_publishes_only_after_the_footer() {
        let bytes = include_bytes!(
            "../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
        );
        let mut decoder = Decoder::default();
        for line in bytes.split_inclusive(|byte| *byte == b'\n') {
            let value: serde_json::Value = serde_json::from_slice(line).unwrap();
            if value["type"] == "scan_request" {
                decoder.begin_scan().unwrap();
                continue;
            }
            if value["type"] == "client_hello" {
                continue;
            }
            for chunk in line.chunks(7) {
                decoder.feed(chunk).unwrap();
            }
            assert_eq!(decoder.capture().is_some(), value["type"] == "scan_footer");
        }
        let capture = decoder.capture().unwrap();
        assert_eq!(capture.lookup(0, 7, true), "resident");
        assert_eq!(capture.lookup(0, 8, true), "not-resident");
        assert_eq!(capture.lookup(0, 8, false), "unknown");
    }
}
