//! Private runtime source boundary, independent of disk-follow ownership.
//!
//! Capability-only delivery deliberately does not connect to a producer. A
//! configured path is retained privately for the authenticated adapter in the
//! next slice; it is never probed, disclosed, or treated as identity evidence.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::model::Vpid;
use serde::{Deserialize, Serialize};

/// Structural scope validation only: VPID types enforce nonnegative IDs and
/// the bound is checked before copying. Identity/membership proof belongs to
/// verified attachment, not this capability-only source. Ordering is retained.
#[derive(Debug)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Reserved private observation interface for ticket 02"
    )
)]
pub(super) struct ValidatedScope {
    pages: Box<[Vpid]>,
    epoch: u64,
}

#[derive(Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Reserved private observation interface for ticket 02"
    )
)]
pub(super) enum ScopeError {
    TooManyPages,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Reserved private observation interface for ticket 02"
    )
)]
impl ValidatedScope {
    pub(super) fn new(pages: &[Vpid], epoch: u64) -> Result<Self, ScopeError> {
        if pages.len() > 512 {
            return Err(ScopeError::TooManyPages);
        }
        Ok(Self {
            pages: pages.into(),
            epoch,
        })
    }

    pub(super) fn pages(&self) -> &[Vpid] {
        &self.pages
    }

    pub(super) fn epoch(&self) -> u64 {
        self.epoch
    }
}

/// No capture has evaluated any requested page. This is neither an empty
/// complete scan nor evidence of nonresidency; ticket 02 supplies real captures.
#[derive(Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Reserved private observation interface for ticket 02"
    )
)]
pub(super) enum ObservationEvidence {
    None,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Reserved private observation interface for ticket 02"
    )
)]
pub(super) struct ObservationBatch {
    pub capability: Capability,
    pub scope: ValidatedScope,
    pub evidence: ObservationEvidence,
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
}

pub(super) struct Broker {
    adapter: Adapter,
    sleep: Scheduler,
}

pub(super) type SleepFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
pub(super) type Scheduler = Arc<dyn Fn(Duration) -> SleepFuture + Send + Sync>;

fn monotonic_sleep(duration: Duration) -> SleepFuture {
    Box::pin(tokio::time::sleep(duration))
}

enum Adapter {
    Disabled,
    DeferredSocket {
        _path: PathBuf,
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
            adapter: socket.map_or(Adapter::Disabled, |path| Adapter::DeferredSocket {
                _path: path,
            }),
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
            sleep,
        }
    }

    pub(super) async fn capabilities(&self) -> Result<Capability, ()> {
        tokio::select! {
            biased;
            () = (self.sleep)(Duration::from_secs(1)) => Err(()),
            capability = self.read_capability() => Ok(capability),
        }
    }

    /// Bounded internal operation, deliberately not an HTTP observation route.
    /// Capability polling stays separate and must never trigger this operation.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Ticket 02 adds the observation caller; metadata polling must remain separate"
        )
    )]
    pub(super) async fn observe(&self, scope: ValidatedScope) -> Result<ObservationBatch, ()> {
        Ok(ObservationBatch {
            capability: self.capabilities().await?,
            scope,
            evidence: ObservationEvidence::None,
        })
    }

    fn read_capability(&self) -> Pin<Box<dyn Future<Output = Capability> + Send + '_>> {
        Box::pin(async move {
            let (state, reason) = match &self.adapter {
                Adapter::Disabled => (CapabilityState::Disabled, "not-requested"),
                Adapter::DeferredSocket { .. } => {
                    (CapabilityState::Unavailable, "attachment-not-implemented")
                }
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
        for observe in [false, true] {
            for milliseconds in [999, 1000, 1001] {
                let clock = ManualScheduler::default();
                let (arrived, arrivals) = std::sync::mpsc::channel();
                let release = std::sync::Arc::new(tokio::sync::Semaphore::new(0));
                let broker =
                    Broker::simulated_with_scheduler(arrived, release.clone(), clock.scheduler());
                let mut request = std::pin::pin!(async {
                    if observe {
                        let scope = ValidatedScope::new(&[], 0).unwrap();
                        broker.observe(scope).await.map(|batch| batch.capability)
                    } else {
                        broker.capabilities().await
                    }
                });
                let mut context = Context::from_waker(Waker::noop());
                assert!(request.as_mut().poll(&mut context).is_pending());
                arrivals.try_recv().unwrap();
                clock.advance(Duration::from_millis(milliseconds));
                if milliseconds < 1000 {
                    assert!(request.as_mut().poll(&mut context).is_pending());
                }
                // At the boundary both adapter and timer are ready: expiry wins.
                release.add_permits(1);
                match request.as_mut().poll(&mut context) {
                    Poll::Ready(Ok(capability)) if milliseconds < 1000 => {
                        assert_eq!(
                            serde_json::to_value(capability).unwrap()["state"],
                            "unavailable"
                        );
                    }
                    Poll::Ready(Err(())) if milliseconds >= 1000 => {}
                    _ => panic!("incorrect capability deadline result at {milliseconds} ms"),
                }
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

    #[test]
    fn bounded_scope_observation_preserves_request_without_fabricating_evidence() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let pages = [
                Vpid::new(VolId::new(1).unwrap(), PageId::new(9).unwrap()),
                Vpid::new(VolId::new(0).unwrap(), PageId::new(3).unwrap()),
            ];
            for socket in [None, Some(PathBuf::from("/private/not-connected.sock"))] {
                let expected_state = if socket.is_some() {
                    "unavailable"
                } else {
                    "disabled"
                };
                let broker = Broker::new(socket);
                let scope = ValidatedScope::new(&pages, 17).unwrap();
                let observation = broker.observe(scope).await.unwrap();
                assert_eq!(observation.scope.pages(), &pages);
                assert_eq!(observation.scope.epoch(), 17);
                assert_eq!(observation.evidence, ObservationEvidence::None);
                let capability = serde_json::to_value(observation.capability).unwrap();
                assert_eq!(capability["state"], expected_state);
                assert_eq!(capability["verification"], "unverified");
            }
        });
    }
}
