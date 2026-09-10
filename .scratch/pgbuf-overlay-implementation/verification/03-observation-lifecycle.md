# Ticket 03 — Observation lifecycle verification

Date: 2026-09-10. Implementation base: `bec360c4b3facd9be6df774b76d1f31be495de0c`.
The consumer revision is the commit containing this record. Producer-owned
wire corpus remains pinned to `f8c068f771ccd141a3c2f08541fe75c84e41ce53`;
no producer protocol or corpus files changed.

## Result

Final gate: **passed** (exit 0). **302 Rust tests, 70 frontend unit tests,
and 13 browser cases passed**; three pre-existing Rust ignores and one
pre-existing Firefox skip remain. All eight ticket-03 lifecycle browser cases
passed. See [the full gate log](03-verify.log).

The gate command is `just verify`: Rust formatting, all Rust tests, Clippy,
static-musl release build and ELF checks, frontend typechecking and unit tests,
generated-artifact reproducibility, dependency advisories, Chromium/Firefox,
embedded-asset checks, metadata and whitespace validation.

## Evidence by contract

| Contract | Executed coverage |
| --- | --- |
| Caller cadence and shared scan floor | `caller_cadence_controls_cache_reuse_and_resume_requires_a_later_scan`; `resume_obeys_the_independent_broker_scan_start_floor_with_a_virtual_scheduler` |
| Resume versus existing work | `a_pre_resume_inflight_capture_cannot_fulfill_resumed_demand`; browser reducer regression rejects a failed resume's stale fallback and preserves its next-request barrier |
| Shared cancellation | `cancellation_preserves_other_callers_and_last_observer_closes_unfinished_scan`: one cancelled caller leaves the surviving caller's capture intact; cancelling the final caller closes the unfinished stream |
| Paused metadata and expiry | `metadata_offers_capture_identity_without_scanning_and_expires_while_paused`; browser metadata offers do not adopt evidence; a dedicated cancellable browser deadline expires evidence independently of the display ticker |
| Restart and identity | `paused_metadata_detects_replaced_producer_and_refuses_until_explicit_retry`; `metadata_preserves_matching_disk_identity_and_invalidates_a_changed_volume`; browser regressions clear old-incarnation evidence before stale-fallback handling and accept invalidation after broker revision reset |
| Failures and age | `producer_errors_have_stable_normalized_reasons_and_only_refusals_stop_retry`; `wall_clock_steps_and_suspension_revoke_broker_cache_age_authority`; browser tests cover 500/1000/2000/4000/8000/8000 ms nominal retry, jitter bounds, full RTT, original age after failure, cache-age monotonicity and uncertain elapsed time |
| Real HTTP | `selected_page_observation_crosses_real_socket_and_http_with_bound_identity`: cadence bounds, cache reuse, forced post-request scan, scope echo, no-store, sanitized output and unchanged inspection session |
| Browser lifecycle | Four scenarios in **each of Chromium and Firefox**: selected-page detail; automatic polling, another-tab offers, hidden silence, resume and expiry; real scripted-producer restart while paused and explicit retry; transient transport retries and protocol incompatibility |
| Late adoption and disk navigation | Browser model tests revoke scope/epoch authority on navigation, overlay, visibility, pause and generation changes; a newer disk route response cannot adopt after pause; same-page generation replacement retains state-only evidence |
| Existing safety ceilings | The full broker/decoder suite retains actual socket timeout checks, canonical exchanges, UID isolation, framing/count/memory/response/admission bounds and malformed-scan retention tests from ticket 02 |

The nine added broker scenarios are in
[`lifecycle_tests.rs`](../../../src/web/observations/lifecycle_tests.rs).
The focused broker command executes **25 tests** including the existing safety
coverage: `cargo +1.97.1 test --locked --lib web::observations -- --nocapture`.
Actual handshake/scan timeout tests use real I/O time; injected clock and
scheduler tests do not substitute for those checks.

Browser scenarios use the actual HTTP server and the pinned scripted AF_UNIX
producer. Test-only SIGHUP changes the producer incarnation while paused.
Transport/protocol failure cases additionally intercept the browser boundary.
Process-wide restart scenarios run serially across browser projects. The
multi-tab scenario waits for rendered content rather than a document load event
that can remain pending alongside a live generation watch.

Visual evidence:

- [Chromium expired evidence](lifecycle-expired-chromium.png)
- [Firefox expired evidence](lifecycle-expired-firefox.png)
- [Chromium selected-page detail](selected-observation-chromium.png)
- [Firefox selected-page detail](selected-observation-firefox.png)

## Review

Independent Standards review: **0 unresolved findings**. Socket ownership,
type and mode policy is shared by handshake and metadata probing; the separate
post-connect peer/socket identity checks remain intact.

Independent Spec review: **0 unresolved findings**. The reviewer rechecked
resume failure, cross-tab incarnation changes, broker revision reset and the
cancellable browser expiry deadline after regression-backed fixes.

## Scope and remaining evidence

Three existing Rust ignores are manual TUI preview/resource-benchmark cases.
The existing Firefox skip is the Chromium-owned parity corpus in
`current-viewer.spec.ts`; all ticket-03 browser cases run in both browsers.
These are not substituted for any lifecycle case.

Real CUBRID producer integration, viewport observations, dense rendering,
manual accessibility review and release performance gates remain in tickets
04–08. This ticket preserves the state-only overlay and does not establish
page-image correspondence, commit visibility, durability or event causality.
