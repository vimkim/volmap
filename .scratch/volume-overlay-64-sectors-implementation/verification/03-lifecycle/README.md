# Ticket 03: mixed-demand lifecycle verification

Implementation baseline: `86f2f50af2c9206153f123cd638d98b269980403`.
This is a local authenticated-fixture correctness workload. It does not qualify
real-producer performance, browser RSS, or input latency for tickets 04/05.

## Workload and evidence

`web/e2e/multitab-observations.spec.ts` opens 1, 8, and 32 independent browser
contexts in Chromium and Firefox, cycling Volume 0, Sector 0, and Page 0:10.
Independent contexts keep each virtual clock and visibility state independent.
All tabs first enable observations concurrently; each pauses after its response.
They then resume one at a time, requiring `after_request: true`. Page cadence is
500 ms; Volume/Sector cadence is 2,000 ms. The workload records each caller's
scope, epoch, generation, response status, capture envelope, and visible state.
It checks response identity per request, without requiring separate callers to
receive the same capture. Both 200 and explicit 429 are valid under contention.
Every tab subsequently navigates through the real disk HTTP route.
A transparent Playwright route hook forwards each original observation request
with `route.fetch()` and returns its exact HTTP response, retaining JSON before
Chromium can discard a canceled response resource. It does not synthesize
observations or bypass the authenticated producer. This trace instrumentation
is for correctness; cancellation ownership is checked separately by the broker
regressions, and this harness must not be used as a latency measurement.

The Rust HTTP test holds the authenticated producer before its footer and sends
32 mixed requests. Exactly eight wait, and all 24 excess requests must return
429 **before** releasing the producer. This handshake proves no extra admission
queue. Disk session/volume navigation and capability metadata succeed during
this stall. The original complete/partial/duplicate nine-caller cases remain.
After releasing the 32-caller wave, a virtual clock advances to 1,000 ms.
A 64-sector Volume request and a Sector request at 2,000-ms cadence both reuse
scan 1 at conservative age 1,101 ms; Page at 500-ms cadence starts scan 2 at
age 101 ms. `mixed-cache.log` records each request/capture pair. This separates
caller-dependent cache selection from the resume barrier workload.

Separate browser scenarios hold a Volume or Sector paused while a Page context
refreshes. Metadata offers newer availability without replacing the paused
page evidence; hidden contexts send no runtime requests; resume requests a new
scan; evidence expires after advancing the browser clock by 31 seconds.

## Acceptance evidence map

| Contract | Tests / artifacts |
| --- | --- |
| 1/8/32 simultaneous and staggered mixed HTTP demand | `multitab-observations.spec.ts`; `multitab-*-*.json`; gated `thirty_two_http_callers_receive_bounded_overload_without_queuing` |
| Eight permits, no queue, disk independence | gated HTTP test; `held_http_response_bodies_keep_admission_until_released` now covers Page, Sector, and 64-sector Volume |
| 2.5-second request/body deadline | `observation_http_deadline_includes_stalled_request_body`; `actual_io_deadlines_bound_stalled_handshake_and_scan` |
| Four metadata permits, independent one-second deadline | `runtime_capability_admission_is_bounded_and_independent_over_http`, `runtime_capability_deadline_releases_its_slot_over_http` |
| Paused metadata, availability only, hidden silence, expiry | new paused browser cases and `paused-*.json/png`; existing `metadata_offers_capture_identity_without_scanning_and_expires_while_paused` |
| Pre-resume scan exclusion, independent 500-ms floor | `a_pre_resume_inflight_capture_cannot_fulfill_resumed_demand`, `resume_obeys_the_independent_broker_scan_start_floor_with_a_virtual_scheduler` |
| Late route/scope/generation/overlay/pause/visibility responses | new Volume/Sector reducer cases, existing viewport test and browser delayed-response cases |
| Restart/identity mismatch, explicit retry, disk preservation | existing browser Sector/Page restart and source-failure cases; broker `metadata_preserves_matching_disk_identity_and_invalidates_a_changed_volume` and restart regression |
| Conservative age, caller threshold, monotonic cache age, clock steps/suspension, retry/jitter | `observations.test.ts`, `runtime.test.ts`, deterministic broker lifecycle tests |
| Cancellation ownership and no accumulated partial absence | `cancellation_preserves_other_callers_and_last_observer_closes_unfinished_scan`, `rotating_truncated_captures_never_accumulate_absence_or_hide_duplicate_ambiguity`; mixed HTTP partial/duplicate cases |
| 128-MiB aggregate accounting and response lifetimes | `aggregate_boundary_counts_retained_inflight_and_response_owners_until_last_drop`, `actual_decoders_cannot_admit_object_caps_that_exceed_the_shared_total`, scoped held-body cases |

The allocation tests distinguish reservations from RSS. Eight responses reserve
16 MiB; session 8 MiB, old and latest captures 32 MiB each, and decoder scratch
16 MiB bring the tested retained combination to 104 MiB. A further 32-MiB
assembly is explicitly refused. After dropping the old capture, assembly can
proceed; surviving parser and response owners remain charged. The existing
maximum-width HTTP case checks 4,096 actual resident result serialization;
ticket 02's capacity calculation places its per-request allocations below the
unchanged 2-MiB reservation. Captures do not accumulate into absence evidence.

## Standards

No documented-standard violations. One **possible Repeated Switches** (low-priority heuristic) suggestion was
to consolidate repeated route-to-expectation choices in the browser test into
a table. The three view-specific assertions remain explicit in this small
workload; this is not a correctness blocker.

## Spec

The initial review found missing mixed-scope cache/refresh selection assertions.
The deterministic real-HTTP follow-up described above closes that gap; the
second review confirmed zero remaining spec findings. No production runtime
defect was identified by the added regressions.

Standards: 0 rule violations, 1 nonblocking heuristic suggestion. Spec: 0 remaining findings.

## Reproduction and handoff

```sh
cargo +1.97.1 test --locked --lib web::observations::
cargo +1.97.1 test --locked --lib http_
mise x node@24.19.0 -- corepack pnpm --dir web exec vitest run src/observations.test.ts
mise x node@24.19.0 -- corepack pnpm --dir web exec playwright test multitab-observations --project=chromium --project=firefox
just verify
```

`broker.log` and `http.log` retain focused results. The final gate is
`verify.log`; the files below intentionally preserve earlier non-passing runs:

| File | Interpretation |
| --- | --- |
| `browser-harness-failures.log` | Early response-body and selected-Page locator harness failures |
| `browser-smoke.log` | Eight-tab pass; paused tooltip comparison wrongly expected freshness to freeze |
| `browser-focused.log` | Nine passes; one Firefox live-navigation wait subsequently corrected |
| `verify-initial-failure.log` | 92 browser passes, one existing skip, dense Firefox navigation wait failure |
| `dense-firefox-navigation-failure.md` | Failure context showing the Volume UI had rendered |

`resident` in the multitab JSON counts mosaic cells. Selected Page has no
mosaic and records zero; its detail-region residency and capture sequence are
asserted separately. These small-fixture browser traces returned 200 for all
recorded demands; the independently gated HTTP test proves 24 explicit 429s.
Full verification results are recorded below when completed.

Tickets 04/05 should reuse the mixed scope/cadence and simultaneous/staggered
phases, replacing the small correctness fixture with the existing 12,288-cell,
4,096-changing-result dense fixture and then a pinned real producer. Measure
bytes, age, rejection counts, browser process-tree RSS, and latency separately;
these correctness traces are not quantitative performance measurements.

Initial harness corrections: frozen-clock browser interactions stalled;
selected Page uses its detail region rather than a mosaic cell; canceled
viewport responses must be excluded before reading their bodies. These were
test harness defects, not evidence of a production lifecycle regression.
The tooltip legitimately changes from fresh to stale while paused; the retained
capture label, rather than the full tooltip, is compared for adoption freeze.

The timeout regression also sends two successive waves of eight incomplete
HTTP bodies. Every request returns 504 and disk navigation remains available;
the second fully admitted wave proves all eight prior timeout permits returned.
`deadline-recovery.log` records the focused pass.

The first full verification run reached 92 browser passes and one pre-existing
skip, then failed only because Firefox's dense Volume `goto(...commit)` stayed
pending after rendering the UI. The test now uses the existing document-location
navigation pattern, with the same 12,288-cell readiness and 4,096-result checks.
`verify-initial-failure.log` retains that failure. The first navigation
adjustment exposed an early pagination access during document replacement;
`verify-navigation-readiness-failure.log` retains it. A visible Volume heading
now precedes the existing cell readiness checks. The final gate is `verify.log`.

| Browser | Paused Volume expiry | Paused Sector expiry |
| --- | --- | --- |
| Chromium | [Screenshot](paused-volume-chromium.png) | [Screenshot](paused-sector-chromium.png) |
| Firefox | [Screenshot](paused-volume-firefox.png) | [Screenshot](paused-sector-firefox.png) |


## Final validation and user-requested additions

The lifecycle slice passed `just verify`: 93 browser tests passed with one
existing skip, and Rust, frontend, generated-artifact and release checks passed.
After the user requested P/S glyphs and a 1 GiB framed-scan ceiling, all Rust
tests passed again (`scan-1g-rust.log`), frontend types and 81 tests passed
(`membership-artifacts.log`), Clippy passed, and the four affected Volume/Sector
browser tests passed in Chromium and Firefox (`membership-browser.log`).
The glyph regression was observed failing before the fix (`membership-red.log`).
Both reviews found no blocking issues; the pinned historical fixture contract
keeps its old limit, with the current override documented separately.

## Live Volume 1 diagnosis

The live viewer at 192.168.4.2:7777 was receiving truncated producer scans.
One 4,096-page request contained 262 resident pages and 3,834 unknown pages
with reason `partial-omission`. Unknown does not establish nonresidency.
A direct authenticated socket capture emitted 285 records, visited 30,752
slots, and ended after 101,203 microseconds with `truncated: true`
(`live-producer-footer.json`). It emitted only 106,974 bytes.
The producer's 100 ms elapsed budget includes serialization and output
backpressure. Its 5 ms polling cadence and small socket send buffer contribute
to limited delivery. The next scan advances the traversal starting position,
so different refreshes can reveal different small subsets of the volume.

This is incomplete producer coverage, not a reverted Volume renderer. The
read-only `diagnose-live-volume.py` and `live-volume-partial-*` files retain
this external failure; they are diagnostic evidence, not a passing CI gate.
No union of different captures was introduced. The user requested increasing
the separate byte ceiling to 1 GiB; producer and consumer code now agree on
that value. This does not resolve the measured 100 ms truncation.
The engine compiled, but installation was deferred by the build script because
its installation environment has an active server and csql session. The live
producer remains unchanged until an authorized restart/install.

Producer validation: all 27 other CTest targets passed, including the native
attachment fixture. The inspector target passed 60 of 61 cases, including
the new 1 GiB boundaries; its only failure was the corpus README checksum
after updating the documented limit. The checksum and aggregate manifest were
updated (corpus revision 3), then that exact failing case passed all 648
assertions. The initial failure and corrected-case output are retained.

Committed log copies trim trailing whitespace and final blank lines for repository whitespace checks; test output content is preserved.
