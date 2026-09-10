//! Deterministic broker demand against an actual authenticated socket.
use super::*;
use crate::inspection::{RuntimeIdentity, RuntimeVolumeIdentity};
use crate::model::{PageId, VolId, Vpid};
use std::io::{BufRead, Write};
use std::os::unix::{fs::PermissionsExt, net::UnixListener};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

struct Producer {
    directory: PathBuf,
    path: PathBuf,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Producer {
    fn start(name: &str) -> Self {
        Self::script(name, move |listener| {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let transcript = include_str!(
                "../../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
            );
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            writeln!(stream, "{}", transcript.lines().nth(1).unwrap()).unwrap();
            for sequence in 1.. {
                request.clear();
                if reader.read_line(&mut request).unwrap_or(0) == 0 {
                    break;
                }
                for line in transcript.lines().skip(3) {
                    if writeln!(
                        stream,
                        "{}",
                        line.replace(
                            "\"scan_seq\":\"1\"",
                            &format!("\"scan_seq\":\"{sequence}\"")
                        )
                    )
                    .is_err()
                    {
                        return;
                    }
                }
            }
        })
    }

    fn script(name: &str, run: impl FnOnce(UnixListener) + Send + 'static) -> Self {
        let directory =
            std::env::temp_dir().join(format!("volmap-lifecycle-{name}-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("producer.sock");
        let listener = UnixListener::bind(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let thread = std::thread::spawn(move || run(listener));
        Self {
            directory,
            path,
            thread: Some(thread),
        }
    }
}
impl Drop for Producer {
    fn drop(&mut self) {
        self.thread.take().unwrap().join().unwrap();
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}
fn identity() -> RuntimeIdentity {
    RuntimeIdentity {
        database_creation: 1,
        volumes: vec![RuntimeVolumeIdentity {
            volid: 0,
            volume_creation: 2,
            device: 3,
            inode: u64::MAX,
        }],
    }
}
fn scope(cadence: u64, barrier: bool) -> ValidatedScope {
    ValidatedScope::new(
        &[Vpid::new(VolId::new(0).unwrap(), PageId::new(7).unwrap())],
        1,
    )
    .unwrap()
    .with_demand(cadence, barrier)
    .unwrap()
}

#[test]
fn caller_cadence_controls_cache_reuse_and_resume_requires_a_later_scan() {
    let producer = Producer::start("cadence");
    let ticks = Arc::new(AtomicU64::new(0));
    let clock = ticks.clone();
    let origin = Instant::now();
    let broker = Broker::with_clock(
        producer.path.clone(),
        Arc::new(move || origin + Duration::from_millis(clock.load(Ordering::SeqCst))),
    );
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for (now, cadence, barrier, sequence, age) in [
                (0, 500, false, "1", 101),
                (399, 500, false, "1", 500),
                (1000, 2000, false, "1", 1101),
                (1000, 500, false, "2", 101),
                (1500, 2000, true, "3", 101),
            ] {
                ticks.store(now, Ordering::SeqCst);
                let response = broker
                    .observe(scope(cadence, barrier), Some(identity()), "1", false)
                    .await
                    .unwrap();
                let json: serde_json::Value = serde_json::from_slice(response.as_ref()).unwrap();
                assert_eq!(
                    json["capture"]["sequence"], sequence,
                    "at {now}, cadence {cadence}"
                );
                assert_eq!(json["capture"]["upper_age_ms"], age);
            }
        });
    drop(broker);
}

#[test]
fn metadata_offers_capture_identity_without_scanning_and_expires_while_paused() {
    let producer = Producer::start("metadata");
    let ticks = Arc::new(AtomicU64::new(0));
    let clock = ticks.clone();
    let origin = Instant::now();
    let broker = Broker::with_clock(
        producer.path.clone(),
        Arc::new(move || origin + Duration::from_millis(clock.load(Ordering::SeqCst))),
    );
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let response = broker
                .observe(scope(500, false), Some(identity()), "1", false)
                .await
                .unwrap();
            let scan: serde_json::Value = serde_json::from_slice(response.as_ref()).unwrap();
            ticks.store(5000, Ordering::SeqCst);
            let metadata = serde_json::to_value(broker.capabilities().await.unwrap()).unwrap();
            assert_eq!(
                metadata["incarnation_binding"],
                scan["capture"]["incarnation_binding"]
            );
            assert_eq!(metadata["capture_identity"], scan["capture"]["identity"]);
            ticks.store(30_000, Ordering::SeqCst);
            let metadata = serde_json::to_value(broker.capabilities().await.unwrap()).unwrap();
            assert!(metadata["capture_identity"].is_null());
            assert_eq!(metadata["reason"], "observation-expired");
        });
    drop(broker);
}

#[test]
fn resume_obeys_the_independent_broker_scan_start_floor_with_a_virtual_scheduler() {
    let producer = Producer::start("floor");
    let ticks = Arc::new(AtomicU64::new(0));
    let clock = ticks.clone();
    let origin = Instant::now();
    let now: session::Clock =
        Arc::new(move || origin + Duration::from_millis(clock.load(Ordering::SeqCst)));
    let elapsed = ticks.clone();
    let sleep: Scheduler = Arc::new(move |duration| {
        elapsed.fetch_add(
            u64::try_from(duration.as_millis()).unwrap(),
            Ordering::SeqCst,
        );
        Box::pin(std::future::ready(()))
    });
    let mut broker = Broker::new(Some(producer.path.clone()));
    broker.session = session::Session::with_scheduler(now, sleep);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            drop(
                broker
                    .observe(scope(500, false), Some(identity()), "1", false)
                    .await
                    .unwrap(),
            );
            ticks.store(100, Ordering::SeqCst);
            let response = broker
                .observe(scope(500, true), Some(identity()), "1", false)
                .await
                .unwrap();
            let json: serde_json::Value = serde_json::from_slice(response.as_ref()).unwrap();
            assert_eq!(json["capture"]["sequence"], "2");
            assert_eq!(ticks.load(Ordering::SeqCst), 500);
            assert_eq!(json["capture"]["upper_age_ms"], 101);
        });
    drop(broker);
}

#[test]
fn wall_clock_steps_and_suspension_revoke_broker_cache_age_authority() {
    for (name, wall) in [("backwards", 9999), ("suspension", 15000)] {
        let producer = Producer::start(name);
        let ticks = Arc::new(AtomicU64::new(0));
        let walls = Arc::new(AtomicU64::new(10000));
        let clock = ticks.clone();
        let wall_clock = walls.clone();
        let origin = Instant::now();
        let now = session::conservative_clock(Arc::new(move || {
            (
                origin + Duration::from_millis(clock.load(Ordering::SeqCst)),
                std::time::SystemTime::UNIX_EPOCH
                    + Duration::from_millis(wall_clock.load(Ordering::SeqCst)),
            )
        }));
        let broker = Broker::with_clock(producer.path.clone(), now);
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                drop(
                    broker
                        .observe(scope(500, false), Some(identity()), "1", false)
                        .await
                        .unwrap(),
                );
                ticks.store(1, Ordering::SeqCst);
                walls.store(wall, Ordering::SeqCst);
                let metadata = serde_json::to_value(broker.capabilities().await.unwrap()).unwrap();
                assert!(metadata["capture_identity"].is_null());
                assert_eq!(metadata["reason"], "observation-expired");
            });
        drop(broker);
    }
}

#[test]
fn cancellation_preserves_other_callers_and_last_observer_closes_unfinished_scan() {
    for keep_waiter in [false, true] {
        let (arrived, arrival) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        let (closed, closure) = tokio::sync::oneshot::channel();
        let producer = Producer::script(
            if keep_waiter {
                "keep-waiter"
            } else {
                "cancel-last"
            },
            move |listener| {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let transcript = include_str!(
                    "../../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
                );
                let mut request = String::new();
                reader.read_line(&mut request).unwrap();
                writeln!(stream, "{}", transcript.lines().nth(1).unwrap()).unwrap();
                request.clear();
                reader.read_line(&mut request).unwrap();
                arrived.send(()).unwrap();
                released.recv_timeout(Duration::from_secs(3)).unwrap();
                if keep_waiter {
                    for line in transcript.lines().skip(3) {
                        writeln!(stream, "{line}").unwrap();
                    }
                }
                request.clear();
                assert_eq!(reader.read_line(&mut request).unwrap(), 0);
                closed.send(()).unwrap();
            },
        );
        let broker = Arc::new(Broker::new(Some(producer.path.clone())));
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let first_broker = broker.clone();
                let first = tokio::spawn(async move {
                    first_broker
                        .observe(scope(500, false), Some(identity()), "1", false)
                        .await
                });
                arrival.await.unwrap();
                let waiter = if keep_waiter {
                    let other = broker.clone();
                    let waiter = tokio::spawn(async move {
                        other
                            .observe(scope(500, false), Some(identity()), "1", false)
                            .await
                    });
                    tokio::task::yield_now().await;
                    Some(waiter)
                } else {
                    None
                };
                first.abort();
                assert!(matches!(first.await, Err(error) if error.is_cancelled()));
                release.send(()).unwrap();
                if let Some(waiter) = waiter {
                    let response = waiter.await.unwrap().unwrap();
                    let json: serde_json::Value =
                        serde_json::from_slice(response.as_ref()).unwrap();
                    assert_eq!(json["capture"]["sequence"], "1");
                    assert_eq!(json["observations"][0]["state"], "resident");
                }
                drop(broker);
                tokio::time::timeout(Duration::from_secs(3), closure)
                    .await
                    .unwrap()
                    .unwrap();
            });
    }
}

#[test]
fn paused_metadata_detects_replaced_producer_and_refuses_until_explicit_retry() {
    let (replace, replaced) = std::sync::mpsc::channel();
    let (ready, replacement_ready) = tokio::sync::oneshot::channel();
    let producer = Producer::script("restart", move |listener| {
        let path = listener
            .local_addr()
            .unwrap()
            .as_pathname()
            .unwrap()
            .to_path_buf();
        let transcript = include_str!(
            "../../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
        );
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        writeln!(stream, "{}", transcript.lines().nth(1).unwrap()).unwrap();
        request.clear();
        reader.read_line(&mut request).unwrap();
        for line in transcript.lines().skip(3) {
            writeln!(stream, "{line}").unwrap();
        }
        replaced.recv_timeout(Duration::from_secs(3)).unwrap();
        std::fs::remove_file(&path).unwrap();
        let replacement = UnixListener::bind(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        ready.send(()).unwrap();
        drop(reader);
        drop(stream);
        drop(listener);
        for retry in [false, true] {
            let (mut stream, _) = replacement.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            request.clear();
            reader.read_line(&mut request).unwrap();
            let transcript = transcript.replace(
                "0123456789abcdef0123456789abcdef",
                "ffffffffffffffffffffffffffffffff",
            );
            writeln!(stream, "{}", transcript.lines().nth(1).unwrap()).unwrap();
            request.clear();
            let count = reader.read_line(&mut request).unwrap();
            if retry {
                assert!(count > 0);
                for line in transcript.lines().skip(3) {
                    writeln!(stream, "{line}").unwrap();
                }
            } else {
                assert_eq!(
                    count, 0,
                    "metadata must neither scan nor accept changed incarnation"
                );
            }
        }
    });
    let broker = Broker::new(Some(producer.path.clone()));
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let first = broker
                .observe(scope(500, false), Some(identity()), "1", false)
                .await
                .unwrap();
            let first: serde_json::Value = serde_json::from_slice(first.as_ref()).unwrap();
            replace.send(()).unwrap();
            replacement_ready.await.unwrap();
            let metadata = serde_json::to_value(broker.capabilities().await.unwrap()).unwrap();
            assert_eq!(metadata["state"], "refused");
            assert_eq!(metadata["reason"], "incarnation-changed");
            assert!(metadata["capture_identity"].is_null());
            let automatic = broker
                .observe(scope(500, false), Some(identity()), "1", false)
                .await
                .unwrap();
            let automatic: serde_json::Value = serde_json::from_slice(automatic.as_ref()).unwrap();
            assert!(automatic["capture"].is_null());
            assert_eq!(automatic["capability"]["state"], "refused");
            let retried = broker
                .observe(scope(500, true), Some(identity()), "1", true)
                .await
                .unwrap();
            let retried: serde_json::Value = serde_json::from_slice(retried.as_ref()).unwrap();
            assert_eq!(retried["capture"]["sequence"], "1");
            assert_ne!(
                retried["capture"]["incarnation_binding"],
                first["capture"]["incarnation_binding"]
            );
            assert_eq!(retried["capability"]["state"], "active");
        });
    drop(broker);
}

#[test]
fn metadata_preserves_matching_disk_identity_and_invalidates_a_changed_volume() {
    let producer = Producer::start("identity-metadata");
    let broker = Broker::new(Some(producer.path.clone()));
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let response = broker
                .observe(scope(500, false), Some(identity()), "1", false)
                .await
                .unwrap();
            let response: serde_json::Value = serde_json::from_slice(response.as_ref()).unwrap();
            let metadata =
                serde_json::to_value(broker.capabilities_for(Some(identity())).await.unwrap())
                    .unwrap();
            assert_eq!(
                metadata["capture_identity"],
                response["capture"]["identity"]
            );
            let mut changed = identity();
            changed.volumes[0].inode = 42;
            let metadata =
                serde_json::to_value(broker.capabilities_for(Some(changed)).await.unwrap())
                    .unwrap();
            assert_eq!(metadata["state"], "refused");
            assert_eq!(metadata["reason"], "identity-mismatch");
            assert!(metadata["capture_identity"].is_null());
        });
    drop(broker);
}

#[test]
fn producer_errors_have_stable_normalized_reasons_and_only_refusals_stop_retry() {
    for (code, state, reason) in [
        ("busy", "unavailable", "busy"),
        ("rate-limited", "unavailable", "rate-limited"),
        ("parameter-off", "unavailable", "parameter-off"),
        ("version-unsupported", "incompatible", "version-unsupported"),
    ] {
        let producer = Producer::script(code, move |listener| {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            writeln!(stream, "{{\"type\":\"error\",\"code\":\"{code}\",\"supported_majors\":[1],\"retry_after_ms\":500}}").unwrap();
        });
        let broker = Broker::new(Some(producer.path.clone()));
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let response = broker
                    .observe(scope(500, false), Some(identity()), "1", false)
                    .await
                    .unwrap();
                let response: serde_json::Value =
                    serde_json::from_slice(response.as_ref()).unwrap();
                assert_eq!(response["capability"]["state"], state);
                assert_eq!(response["capability"]["reason"], reason);
                assert!(response["capture"].is_null());
                if state == "incompatible" {
                    let response = broker
                        .observe(scope(500, false), Some(identity()), "1", false)
                        .await
                        .unwrap();
                    let response: serde_json::Value =
                        serde_json::from_slice(response.as_ref()).unwrap();
                    assert_eq!(response["capability"]["state"], "incompatible");
                }
            });
        drop(broker);
    }
}

#[test]
fn a_pre_resume_inflight_capture_cannot_fulfill_resumed_demand() {
    let (arrived, arrival) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let ticks = Arc::new(AtomicU64::new(0));
    let producer_clock = ticks.clone();
    let producer = Producer::script("inflight-resume", move |listener| {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
        let transcript = include_str!(
            "../../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
        );
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        writeln!(stream, "{}", transcript.lines().nth(1).unwrap()).unwrap();
        request.clear();
        reader.read_line(&mut request).unwrap();
        arrived.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(3)).unwrap();
        producer_clock.store(600, Ordering::SeqCst);
        for sequence in 1..=2 {
            if sequence == 2 {
                request.clear();
                assert!(reader.read_line(&mut request).unwrap() > 0);
            }
            for line in transcript.lines().skip(3) {
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
    });
    let clock = ticks.clone();
    let origin = Instant::now();
    let broker = Arc::new(Broker::with_clock(
        producer.path.clone(),
        Arc::new(move || origin + Duration::from_millis(clock.load(Ordering::SeqCst))),
    ));
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let first_broker = broker.clone();
            let first = tokio::spawn(async move {
                first_broker
                    .observe(scope(500, false), Some(identity()), "1", false)
                    .await
            });
            arrival.await.unwrap();
            ticks.store(100, Ordering::SeqCst);
            let resumed_broker = broker.clone();
            let resumed = tokio::spawn(async move {
                resumed_broker
                    .observe(scope(500, true), Some(identity()), "1", false)
                    .await
            });
            tokio::task::yield_now().await;
            release.send(()).unwrap();
            drop(first.await.unwrap().unwrap());
            let response = resumed.await.unwrap().unwrap();
            let response: serde_json::Value = serde_json::from_slice(response.as_ref()).unwrap();
            assert_eq!(response["capture"]["sequence"], "2");
            assert_eq!(response["capture"]["upper_age_ms"], 101);
        });
    drop(broker);
}

#[test]
fn rotating_truncated_captures_never_accumulate_absence_or_hide_duplicate_ambiguity() {
    let producer = Producer::script("rotation", move |listener| {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
        let transcript = include_str!(
            "../../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
        );
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        writeln!(stream, "{}", transcript.lines().nth(1).unwrap()).unwrap();
        // Script a stable six-slot pool: truncated scans advance beyond their
        // two visited slots, then a complete scan has a duplicate VPID.
        for sequence in 1..=4 {
            request.clear();
            reader.read_line(&mut request).unwrap();
            let binding = format!(
                "\"incarnation\":\"0123456789abcdef0123456789abcdef\",\"scan_seq\":\"{sequence}\""
            );
            writeln!(
                stream,
                "{{\"type\":\"scan_header\",{binding},\"start_time_us\":\"1\"}}"
            )
            .unwrap();
            let pages = if sequence == 4 {
                vec![0, 1, 2, 3, 4, 4]
            } else {
                vec![(sequence - 1) * 2, (sequence - 1) * 2 + 1]
            };
            for page in &pages {
                writeln!(
                    stream,
                    "{{\"type\":\"page\",{binding},\"volid\":0,\"pageid\":{page}}}"
                )
                .unwrap();
            }
            writeln!(stream, "{{\"type\":\"scan_footer\",{binding},\"end_time_us\":\"2\",\"record_count\":{},\"visited_slots\":{},\"truncated\":{}}}", pages.len(), pages.len(), sequence != 4).unwrap();
        }
    });
    let ticks = Arc::new(AtomicU64::new(0));
    let clock = ticks.clone();
    let origin = Instant::now();
    let broker = Broker::with_clock(
        producer.path.clone(),
        Arc::new(move || origin + Duration::from_millis(clock.load(Ordering::SeqCst))),
    );
    let pages: Vec<_> = (0..7)
        .map(|page| Vpid::new(VolId::new(0).unwrap(), PageId::new(page).unwrap()))
        .collect();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for sequence in 1..=4 {
                ticks.store(sequence * 2000, Ordering::SeqCst);
                let response = broker
                    .observe(
                        ValidatedScope::new(&pages, sequence).unwrap(),
                        Some(identity()),
                        "1",
                        false,
                    )
                    .await
                    .unwrap();
                let response: serde_json::Value =
                    serde_json::from_slice(response.as_ref()).unwrap();
                assert_eq!(response["evaluated_count"], 7);
                assert_eq!(response["producer_complete"], sequence == 4);
                for page in 0..7 {
                    let expected = if sequence == 4 {
                        match page {
                            0..=3 => "observed-resident",
                            4 => "duplicate-vpid",
                            _ => "observed-not-resident",
                        }
                    } else if page / 2 == usize::try_from(sequence - 1).unwrap() {
                        "observed-resident"
                    } else {
                        "partial-omission"
                    };
                    assert_eq!(response["observations"][page]["reason"], expected);
                }
            }
        });
    drop(broker);
}
