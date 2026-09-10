# 03: Handle pause, resume, failures, and producer restarts

**What to build:** An operator can leave the selected-Page observation running, pause and resume it, or encounter connection failures and engine restarts without stale evidence becoming fresh or being attributed to the wrong source.

**Blocked by:** 02 — Observe the selected page securely.

**Status:** complete

- [x] Add default selected-Page polling at 500 ms through the shared broker. Respect the independent 500 ms broker scan-start floor rather than confusing it with the producer's 100 ms floor. Reuse a capture only within the requesting caller's cadence or join refresh demand; never accumulate missed ticks or historical captures.
- [x] Transient retry uses nominal delays 0.5, 1, 2, 4 and 8 seconds, then eight seconds, with +/-20% jitter and an actual maximum of 9.6 seconds. Reset after a valid scan. Connectivity status and retained, aged evidence remain separate.
- [x] Interpret version-unsupported, busy, rate-limited and incarnation-changed through stable normalized capability/reason behavior. Parameter-off is only defensive compatibility; socket absence does not prove that setting. Peer/identity refusal and protocol incompatibility clear evidence and stop automatic retry until an explicit operator retry.
- [x] Hidden tabs send no runtime requests. Paused tabs stop adoption of disk generations and runtime observations and check capability metadata only every five seconds; these checks cause neither scans nor adoption. Another tab's newer capture is only an availability indication while paused.
- [x] Resume requires a scan started after resume, respecting the shared scan-start floor; an older cache hit or pre-resume in-flight scan cannot fulfill it. Route, scope, overlay and pause changes revoke incompatible late responses even if cancellation races.
- [x] Producer incarnation changes clear retained evidence and adoption authority even while paused. Disk-generation changes prevent incompatible generation-scoped late adoption but do not alone erase retained state for the same proven database/volume identity; matching generations/VPIDs never imply image correspondence.
- [x] Maintain conservative monotonic age across cache reuse and failed requests. Freshness is relative to each caller's expected cadence and requires known age within two intervals. Wall-clock steps never renew age; uncertain elapsed time after suspension revokes freshness. Enforce 30-second broker/browser eviction while paused and display an expired explanation rather than a history.
- [x] Preserve all authentication, framing, memory, timeout and HTTP limits from ticket 02. Polling or reconnecting cannot bypass them, change inspection outcomes/diagnostics or disturb ordinary disk navigation.
- [x] Demonstrate the complete flow with deterministic clock/scheduler broker and browser-effect tests, real HTTP coverage and Chromium/Firefox lifecycle scenarios: retries, refusal/explicit retry, differing cadences, hidden/pause/resume, another-tab newer offers, late responses, clock steps/suspension, expiry and restart while paused. Keep actual socket timeout checks separate from virtual-time tests.


## Comments

2026-09-10 — Implemented selected-Page polling, bounded jittered retries,
metadata-only pause checks, hidden-tab suppression and post-resume scan demand.
The shared broker preserves other callers on cancellation, cancels the last
observer's unfinished scan, detects producer/volume identity changes, and keeps
cache eligibility separate from the independent scan-start floor. Browser
evidence retains its original conservative age and has a cancellable 30-second
expiry deadline. Late responses cannot adopt after authority is revoked.

`just verify` passed: 302 Rust tests, 70 frontend tests and 13 browser cases
(three existing Rust ignores and one existing Firefox skip). All new lifecycle
scenarios passed in both browsers. Independent Standards and Spec reviews
finished with zero unresolved findings after regression-backed fixes.
See [the verification record](../verification/03-observation-lifecycle.md)
for named cases, raw gate output, screenshots and the boundary with later
implementation/release tickets.
