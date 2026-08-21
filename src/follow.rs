//! Live follow: deciding when a mutable input is worth re-reading, and holding
//! the generations produced by doing so.
//!
//! The offline contract treats a changed input as the end of a session. A
//! running database changes constantly, so following one needs a different
//! answer: re-read it, number the readings, and say plainly which reading the
//! operator is looking at. This module owns that decision and that numbering;
//! it knows nothing about HTTP.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::watch;

use crate::inspection::{
    CancelToken, GraphView, Inspection, OpenRequest, ResourcePolicy, RevisionSelector,
};
use crate::model::SnapshotValidity;
use crate::source::{self, InputFingerprint};

/// How long a long-poll waiter blocks before reporting no change.
pub const WATCH_TIMEOUT: Duration = Duration::from_secs(25);

/// Tuning for the follower's poll-and-debounce loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FollowConfig {
    /// How often the input fingerprint manifest is read.
    pub poll_interval: Duration,
    /// How long the manifest must hold still before a re-read is worthwhile.
    pub quiet_period: Duration,
    /// The staleness ceiling: re-read even if the manifest never goes quiet.
    pub max_defer: Duration,
    /// A floor on the gap between re-reads, independent of scan cost.
    pub min_idle: Duration,
    /// How many recent generations stay addressable.
    pub retain: usize,
}

impl Default for FollowConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(500),
            quiet_period: Duration::from_millis(300),
            max_defer: Duration::from_secs(3),
            min_idle: Duration::from_millis(250),
            retain: 4,
        }
    }
}

/// Everything the re-read decision depends on, as elapsed times rather than
/// clock readings, so the policy is a pure function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RescanInputs {
    pub change_pending: bool,
    pub since_last_change: Duration,
    pub since_first_change: Duration,
    pub since_last_scan: Duration,
    pub last_scan_duration: Duration,
}

/// Decides whether an observed input change has earned a re-read.
///
/// Two independent brakes apply. A settle brake waits for the manifest to hold
/// still, but yields to `max_defer` so a continuously written database is still
/// followed rather than deferred forever. A load brake keeps the gap between
/// re-reads at least as long as the last re-read took, which holds a large
/// volume under a fifty-percent duty cycle without a per-database knob.
#[must_use]
pub fn should_rescan(inputs: RescanInputs, config: &FollowConfig) -> bool {
    if !inputs.change_pending {
        return false;
    }
    let settled = inputs.since_last_change >= config.quiet_period
        || inputs.since_first_change >= config.max_defer;
    let idle_required = config.min_idle.max(inputs.last_scan_duration);
    settled && inputs.since_last_scan >= idle_required
}

/// One complete reading of a live input, with its own inspection-revision
/// chain. Generations replace one another; they are not revisions of one
/// another.
#[derive(Debug)]
struct Generation {
    number: u64,
    views: BTreeMap<u64, GraphView>,
    latest_revision: u64,
    fingerprint: Option<InputFingerprint>,
    observed_at: SystemTime,
    scan_duration: Duration,
}

/// The identity and standing of the reading a request was answered from.
#[derive(Clone, Debug)]
pub struct Reading {
    pub generation: u64,
    pub view: GraphView,
    /// `valid`, `torn` as scanned, or `superseded` once the input has moved on.
    pub validity: SnapshotValidity,
    /// When this generation was read.
    pub observed_at_unix_seconds: u64,
    /// When the input it read last changed on disk, which is a different
    /// question and usually an earlier answer.
    pub input_modified_unix_seconds: Option<u64>,
    pub scan_duration: Duration,
}

#[derive(Debug)]
struct LiveState {
    generations: BTreeMap<u64, Generation>,
    current: u64,
    /// Set once a manifest change is observed, cleared when it is serviced.
    change_pending: bool,
}

/// The live session's generations and the follower's change state.
#[derive(Debug)]
pub struct LiveSource {
    state: RwLock<LiveState>,
    notify: watch::Sender<u64>,
    config: FollowConfig,
    following: bool,
}

/// A failure to read the session state. It only happens if a holder panicked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionUnavailable;

impl LiveSource {
    /// Seeds the session with its first generation.
    #[must_use]
    pub fn new(view: GraphView, config: FollowConfig, following: bool) -> Arc<Self> {
        // Recorded whether or not this session follows: an immutable reading
        // still has a disk time worth reporting, and only the watcher cares
        // about comparing manifests.
        let fingerprint = Some(view.source_fingerprint());
        let mut generations = BTreeMap::new();
        let latest_revision = view.overview().revision.get();
        generations.insert(
            0,
            Generation {
                number: 0,
                views: BTreeMap::from([(latest_revision, view)]),
                latest_revision,
                fingerprint,
                observed_at: SystemTime::now(),
                scan_duration: Duration::ZERO,
            },
        );
        Arc::new(Self {
            state: RwLock::new(LiveState {
                generations,
                current: 0,
                change_pending: false,
            }),
            notify: watch::channel(0).0,
            config,
            following,
        })
    }

    #[must_use]
    pub const fn config(&self) -> &FollowConfig {
        &self.config
    }

    #[must_use]
    pub const fn following(&self) -> bool {
        self.following
    }

    /// A receiver that fires whenever a new generation is published.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.notify.subscribe()
    }

    /// The reading a fresh request should be answered from.
    pub fn current(&self) -> Result<Reading, SessionUnavailable> {
        let state = self.state.read().map_err(|_| SessionUnavailable)?;
        Self::reading(&state, state.current).ok_or(SessionUnavailable)
    }

    /// The reading a request already bound to `generation` should continue on,
    /// or `None` once that generation has fallen out of the retention window.
    pub fn retained(&self, generation: u64) -> Result<Option<Reading>, SessionUnavailable> {
        let state = self.state.read().map_err(|_| SessionUnavailable)?;
        Ok(Self::reading(&state, generation))
    }

    fn reading(state: &LiveState, generation: u64) -> Option<Reading> {
        let entry = state.generations.get(&generation)?;
        let view = entry.views.get(&entry.latest_revision)?.clone();
        Some(Reading {
            generation: entry.number,
            validity: Self::standing(state, entry, &view),
            observed_at_unix_seconds: entry
                .observed_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            input_modified_unix_seconds: entry
                .fingerprint
                .as_ref()
                .and_then(InputFingerprint::newest_modified_unix_seconds),
            scan_duration: entry.scan_duration,
            view,
        })
    }

    /// A generation's standing, which the generation itself cannot know.
    ///
    /// A torn scan stays torn: that is a fact about how it was read. Anything
    /// else is superseded once a newer generation exists or the follower has
    /// seen an unserviced change, because the input has moved past it.
    fn standing(state: &LiveState, entry: &Generation, view: &GraphView) -> SnapshotValidity {
        let scanned = view.overview().validity;
        if matches!(
            scanned,
            SnapshotValidity::Invalidated | SnapshotValidity::Torn
        ) {
            return scanned;
        }
        if entry.number < state.current || state.change_pending {
            return SnapshotValidity::Superseded;
        }
        scanned
    }

    /// Publishes a re-read as the next generation and evicts beyond the
    /// retention window. Returns the new generation number.
    pub fn publish(
        &self,
        view: GraphView,
        scan_duration: Duration,
    ) -> Result<u64, SessionUnavailable> {
        let number = {
            let mut state = self.state.write().map_err(|_| SessionUnavailable)?;
            let number = state.current.saturating_add(1);
            let latest_revision = view.overview().revision.get();
            state.generations.insert(
                number,
                Generation {
                    number,
                    fingerprint: Some(view.source_fingerprint()),
                    views: BTreeMap::from([(latest_revision, view)]),
                    latest_revision,
                    observed_at: SystemTime::now(),
                    scan_duration,
                },
            );
            state.current = number;
            state.change_pending = false;
            let retain = self.config.retain.max(1);
            while state.generations.len() > retain {
                let Some(oldest) = state.generations.keys().next().copied() else {
                    break;
                };
                if oldest == number {
                    break;
                }
                state.generations.remove(&oldest);
            }
            number
        };
        // Ignored deliberately: with no live-poll waiters there is nobody to
        // tell, which is not a failure to publish.
        let _ = self.notify.send(number);
        Ok(number)
    }

    /// Adds an enrichment result as the next revision of `generation`.
    ///
    /// Returns `false` if that generation is no longer the one being extended,
    /// which means a re-read overtook the enrichment and its result belongs to
    /// a reading nobody is looking at any more.
    pub fn publish_revision(
        &self,
        generation: u64,
        view: GraphView,
    ) -> Result<bool, SessionUnavailable> {
        let mut state = self.state.write().map_err(|_| SessionUnavailable)?;
        let Some(entry) = state.generations.get_mut(&generation) else {
            return Ok(false);
        };
        let revision = view.overview().revision.get();
        if revision <= entry.latest_revision {
            return Ok(false);
        }
        entry.views.insert(revision, view);
        entry.latest_revision = revision;
        Ok(true)
    }

    /// Records whether the follower currently sees an unserviced input change.
    pub fn note_change_pending(&self, pending: bool) -> Result<(), SessionUnavailable> {
        let mut state = self.state.write().map_err(|_| SessionUnavailable)?;
        state.change_pending = pending;
        Ok(())
    }

    /// The fingerprint manifest of the current generation, for comparison with
    /// a fresh reading of the input.
    pub fn current_fingerprint(&self) -> Result<Option<InputFingerprint>, SessionUnavailable> {
        let state = self.state.read().map_err(|_| SessionUnavailable)?;
        Ok(state
            .generations
            .get(&state.current)
            .and_then(|entry| entry.fingerprint.clone()))
    }
}

/// Runs the poll-and-debounce loop for one live session until the process ends.
///
/// Every failure mode here is transient by nature — a manifest being rewritten,
/// a volume briefly absent, a scan that hits a resource ceiling — so the loop
/// reports and keeps following rather than tearing the session down.
pub async fn follow(source: Arc<LiveSource>, request: OpenRequest, policy: ResourcePolicy) {
    let config = *source.config();
    let mut ticker = tokio::time::interval(config.poll_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut pending: Option<PendingChange> = None;
    let mut last_scan_at = Instant::now();
    let mut last_scan_duration = Duration::ZERO;

    loop {
        ticker.tick().await;
        let Ok(Some(known)) = source.current_fingerprint() else {
            continue;
        };
        let input = request.input.clone();
        let observed = match tokio::task::spawn_blocking(move || source::fingerprint(&input)).await
        {
            Ok(Ok(value)) => value,
            // An unreadable manifest is not evidence of a change - the
            // database may be mid-write. The next tick asks again.
            Ok(Err(_)) => continue,
            Err(_) => return,
        };
        let now = Instant::now();

        if observed == known {
            if pending.take().is_some() {
                let _ = source.note_change_pending(false);
            }
            continue;
        }

        // The manifest differs from the generation on display. Track when it
        // last *moved*, which is what the quiet period is about, separately
        // from when it first left the published generation behind.
        match &mut pending {
            None => {
                pending = Some(PendingChange {
                    first_at: now,
                    moved_at: now,
                    observed,
                });
                let _ = source.note_change_pending(true);
            }
            Some(change) => {
                if change.observed != observed {
                    change.moved_at = now;
                    change.observed = observed;
                }
            }
        }
        let Some(change) = pending.as_ref() else {
            continue;
        };

        let inputs = RescanInputs {
            change_pending: true,
            since_last_change: now.saturating_duration_since(change.moved_at),
            since_first_change: now.saturating_duration_since(change.first_at),
            since_last_scan: now.saturating_duration_since(last_scan_at),
            last_scan_duration,
        };
        if !should_rescan(inputs, &config) {
            continue;
        }

        let scan_request = request.clone();
        let began = Instant::now();
        let scanned = tokio::task::spawn_blocking(move || {
            Inspection::open_live(&scan_request, policy, &CancelToken::new(), None).and_then(
                |inspection| {
                    inspection
                        .view(RevisionSelector::Latest)
                        .map_err(|_| crate::inspection::OpenFailure::FactStore)
                },
            )
        })
        .await;
        last_scan_duration = began.elapsed();
        last_scan_at = Instant::now();
        match scanned {
            Ok(Ok(view)) => {
                pending = None;
                if source.publish(view, last_scan_duration).is_err() {
                    return;
                }
            }
            // Keep following: the next attempt may land between writes, and the
            // last good generation stays on display meanwhile.
            Ok(Err(error)) => {
                eprintln!(
                    "WARNING: re-reading the input failed; keeping generation on display: {error}"
                );
            }
            Err(_) => return,
        }
    }
}

/// An input change observed but not yet serviced by a re-read.
#[derive(Debug)]
struct PendingChange {
    /// When the manifest first differed from the published generation.
    first_at: Instant,
    /// When the manifest last changed value, which the quiet period measures.
    moved_at: Instant,
    observed: InputFingerprint,
}

#[cfg(test)]
mod tests {
    use super::{FollowConfig, RescanInputs, should_rescan};
    use std::time::Duration;

    const fn inputs() -> RescanInputs {
        RescanInputs {
            change_pending: true,
            since_last_change: Duration::from_secs(10),
            since_first_change: Duration::from_secs(10),
            since_last_scan: Duration::from_secs(10),
            last_scan_duration: Duration::ZERO,
        }
    }

    #[test]
    fn no_pending_change_never_rescans() {
        let config = FollowConfig::default();
        assert!(!should_rescan(
            RescanInputs {
                change_pending: false,
                ..inputs()
            },
            &config
        ));
    }

    #[test]
    fn a_still_unsettled_change_waits_for_the_quiet_period() {
        let config = FollowConfig::default();
        assert!(!should_rescan(
            RescanInputs {
                since_last_change: Duration::from_millis(10),
                since_first_change: Duration::from_millis(10),
                ..inputs()
            },
            &config
        ));
    }

    #[test]
    fn a_never_quiet_input_is_still_read_at_the_staleness_ceiling() {
        let config = FollowConfig::default();
        assert!(should_rescan(
            RescanInputs {
                since_last_change: Duration::ZERO,
                since_first_change: config.max_defer,
                ..inputs()
            },
            &config
        ));
    }

    #[test]
    fn the_gap_between_reads_is_at_least_the_last_read_cost() {
        let config = FollowConfig::default();
        let slow = RescanInputs {
            last_scan_duration: Duration::from_secs(4),
            since_last_scan: Duration::from_secs(2),
            ..inputs()
        };
        assert!(!should_rescan(slow, &config));
        assert!(should_rescan(
            RescanInputs {
                since_last_scan: Duration::from_secs(4),
                ..slow
            },
            &config
        ));
    }

    #[test]
    fn a_cheap_read_still_respects_the_idle_floor() {
        let config = FollowConfig::default();
        assert!(!should_rescan(
            RescanInputs {
                last_scan_duration: Duration::ZERO,
                since_last_scan: Duration::from_millis(10),
                ..inputs()
            },
            &config
        ));
    }
}
