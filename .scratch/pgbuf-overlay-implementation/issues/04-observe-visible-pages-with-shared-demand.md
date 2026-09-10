# 04: Observe visible pages with shared, bounded demand

**What to build:** An operator can observe the selected Page and visible sectors across multiple tabs, with explicit coverage and predictable resource use rather than one engine scan per tab or silently sampled pages.

**Blocked by:** 02 — Observe the selected page securely.

**Status:** complete

- [x] Extend normalized requests and browser presentation to selected-first ordered scopes, followed by nearest visible-sector pages and physical order. At the 512-VPID limit, rotate the non-selected remainder explicitly and show reduced admission/rotation; do not silently exclude the selected Page or claim full viewport evaluation.
- [x] Use a two-second visible-page cadence and a 500 ms selected-page cadence, with one connection, latest-scan cache and in-flight refresh per attached session. Cache eligibility is per caller's own cadence, and scan starts remain at least 500 ms apart. No per-tab scan, connection, missed-tick backlog or history is introduced.
- [x] Return requested/evaluated counts and accepted scope/epoch per waiter independently of producer completeness. Fully evaluated browser scope does not imply a complete producer scan; partial or unevaluated omissions remain unknown. Duplicate ambiguity and complete-scan omission semantics survive batching unchanged.
- [x] Coalesce admitted requests without replacing their scope authority. One caller cancelling cannot cancel another's demand; stop unnecessary work when the last observer leaves. No claim of starvation-free service for arbitrarily many clients is made.
- [x] Enforce eight observation slots including waiters, no queue, 2.5-second deadline and HTTP 429 overload; capabilities retain four independent slots and a one-second deadline. Keep body/response caps at 64 KiB/1 MiB and runtime admission independent of disk requests.
- [x] Under concurrent demand, remain within 48 MiB per decoded scan and 128 MiB total accounted broker allocation, including retained, in-flight, old response-held captures and serialization. Prove that object caps cannot collectively exceed total admission and that no whole-wire buffering or spill is introduced.
- [x] Show coverage/overload and useful available observations at both Volume and Sector scopes using existing semantic presentation; the full visual encoding follows in ticket 05. Existing selected-Page age, expiry and late-response safeguards apply throughout.
- [x] Verify via scripted producer plus real HTTP and browser tests: simultaneous tabs, different cadences, viewport churn, selected priority, overflow rotation, independently validated waiters, cancellation isolation, no-demand shutdown, overload and unaffected concurrent disk requests. Exercise below/at/above scope, response, admission and memory caps, including retained/in-flight/response peaks.
- [x] Test valid truncated rotating captures, exact-cap complete captures and stable-pool rotation progress. Never union partial scans into completeness or absence evidence. Consumer tests recognize the producer contract of at most one visit per slot and rotation beyond the visited span; engine traversal implementation remains owned by CBRD-27398.
- [x] This slice can be developed and verified independently of ticket 03 using explicit requests and controlled visibility/cadence fixtures. It must use the same shared broker scheduling contract so later lifecycle integration preserves all admission and adoption invariants.


## Comments

2026-09-10 — Implemented bounded visible-page demand for Volume and Sector,
selected-first scope construction, explicit overflow rotation and per-row
browser validation. Existing shared broker scheduling and allocation ownership
are retained and exercised with mixed-cadence/scope waiters, real HTTP overload
and concurrent disk requests, rotating truncated captures and aggregate memory
admission. Shared metadata renders coverage, conservative age and limitations;
HTTP 429 has an explicit overload message. Full visual encoding remains ticket
05.

`just verify` passed: 306 Rust tests, 73 frontend tests and 17 browser cases
(three existing Rust ignores and one existing Firefox skip). All 12
producer-backed browser cases passed. Independent Standards and Spec reviews
have zero unresolved findings. See [the verification record](../verification/04-shared-visible-demand.md)
for named boundary tests, the allocation argument, full gate log and screenshots.
