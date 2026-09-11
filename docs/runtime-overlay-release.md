# Page-buffer overlay release evidence

Release readiness is **open**. Ticket 07's real-engine functional verification is
complete; ticket 06 still needs named manual screen-reader and visual reviews.
The dedicated-host performance matrix has not run. The
[delivery manifest](../.scratch/pgbuf-overlay-implementation/verification/08-release-readiness/manifest.json)
indexes inherited evidence and unexecuted gates. A passing local suite or a
`ready-for-agent` triage label is not release approval.

## Prepare a measurement campaign

Record the dedicated host's name, CPU model and logical cores, RAM, OS/kernel,
CPU affinity/governor, storage/filesystem, browser versions, viewport and competing
processes. Reserve it for the campaign; do not infer dedication from an old host
name. Record producer base and implementation commits, consumer commit, clean/dirty
state, source and executable hashes (including loaded engine libraries), build
commands/options, toolchain, testcase revision and corpus manifest/hash. CUBRID's
project release configuration is RelWithDebInfo; Volmap uses the optimized musl
release binary. The unmodified comparison build must use the same producer base,
compiler and options as its corresponding implementation build.

Before execution, supply the workload driver and revision, deterministic seed,
restorable initial database, transaction mix, concurrency, dataset size and
residency/dirty/eviction preconditions. Specify the release producer/profile
combinations being measured; functional evidence for both profiles is already
required separately. Keep existing synchronized native fixtures for semantic
checks; do not insert observer hot-path instrumentation or a shipped control
endpoint. Failed preconditions make a run inconclusive.

The matrix is the product of these axes for each declared release configuration
and browser. Record Chromium and Firefox separately.

| Axis | Cases |
| --- | --- |
| Pool / 16 KiB slots | 512 MiB / 32,768; 1 GiB / 65,536; 4 GiB / 262,144 |
| Workload | Idle; read-heavy; write/flush-heavy; churn |
| Browser tabs | 1; 8; 32, with matched disabled tabs |
| Scenario | Steady; stalled output; rotating partial coverage; pause/resume; hidden/resume; clock discontinuity; producer restart |
| Comparison | Unmodified → parameter-off; parameter-off → enabled without demand; parameter-off → enabled with demand |

This enumerates 252 workload/tab/scenario cells before browser, build and
comparison expansion. Ten pairs are required **per performance case**, not ten
runs shared across the matrix. Controls for scenarios with no inspector activity
still retain the same workload, schedule and tab count. Do not quietly omit cells
that do not naturally apply: explain the precondition and disposition and keep
required missing/inconclusive cases open. For producer rotation use a pool above
the visit cap; record full traversal at/below the cap rather than fabricating a
partial scan. Viewport coverage and producer traversal are separate dimensions.

Each arm has 60 seconds warmup and at least 300 seconds measurement, extended to
at least 100,000 completed transactions where applicable. Idle records no
transaction metrics, with that reason explicit; it still requires observation,
CPU, memory and responsiveness measurements. Alternate or randomize pair order
with a recorded seed. Reset to the same initial database and reproduce workload,
state, concurrency, browser actions and configuration between arms; only the
specified inspector activity/build comparison changes. Record every attempt,
including aborted setup, contamination and failed runs.

## Collect and decide

Preserve timestamped transaction latency samples and completed counts, per-process
CPU time/wall time, sampled RSS with interval/process-tree membership, allocator
charges, scan counters, requested/evaluated pages, visited/emitted slots, complete
footers and failure counts. Preserve browser input and visible-update timestamps,
refresh request and validated-publication timestamps, cached HTTP request and
completed-response timestamps, and concurrent disk-inspection request/response
samples. Pair disabled RSS measurements at identical tab counts. Summed process
RSS can double-count shared mappings and sampling can miss peaks; disclose both
rather than substituting heap allocation or calling a sampled maximum exact.

Measure these gates independently; retain a per-case result:

| Gate | Required result |
| --- | --- |
| Throughput loss | 95% confidence upper bound ≤ 2% |
| Transaction p99 increase | 95% confidence upper bound ≤ 5% |
| Complete scans, idle/read-heavy 1 GiB reference | ≥ 99%; report numerator, denominator, failures and occupancy |
| Refresh demand → validated publication p95 | ≤ 250 ms |
| Cached HTTP request → completed response p95 | ≤ 25 ms |
| Concurrent disk-inspection p95 regression | ≤ 5% |
| Producer / broker CPU at default cadence | ≤ 20% / 50% of one logical core, using CPU-time / wall-time |
| Incremental peak RSS: producer / broker / browser | ≤ 16 MiB / 192 MiB / 64 MiB per tab |
| Decoded scan / total broker allocation | ≤ 48 MiB / 128 MiB, independent of RSS |
| Actual rendered density and per-browser visible-update p95 | ≥ 10,000 laid-out page cells; ≤ 100 ms in each browser |

The browser allowance follows the [2026-09-11 budget revision](../.scratch/pgbuf-overlay-implementation/browser-memory-budget-revision.md):
ordinary developer PCs with 1–8 tabs are the primary usage. The 1/8/32-tab
matrix remains required; local density checks do not establish multi-tab or
repeated-use qualification. Historical 32 MiB verdicts remain unchanged.

Do not divide completeness by successful scans only or accept an empty fast scan
as the loaded reference case. Above the slot cap, show truthful partial coverage;
never combine rotating captures into a nonresidency proof or raise budgets.
Fault-scenario timing includes failed/deadline requests in the raw record; successful
response percentiles alone cannot establish that the scenario passed. Label which
requests were deliberately stalled and report recovery separately.

Pre-register the confidence method in the run record before collecting results.
Use pairs as independent units, not individual correlated transactions. Define
pair throughput loss as `100 * (1 - enabled_tps / baseline_tps)` and pair p99
increase as `100 * (enabled_p99 / baseline_p99 - 1)`. Retain each arm's sample
count, percentile calculation and pair effects. Record the analysis tool/version,
method and assumptions, confidence direction, random seed/resample count if used,
and commands that recreate the upper bounds from raw samples. A method that cannot
support its assumptions, a zero denominator, insufficient transactions or a bound
above the ceiling is not a pass. Method selection and executable workload/metric
collection are still missing inputs; this document is a protocol, not an
implemented benchmark or computed confidence interval. Do not pool failing cases,
browsers, parameter-off checks or enabled-without-demand checks into a passing mean.

## Evidence and reruns

Use the manifest's `run_record_template` for each executed case. Replace missing
values with actual records, not planned commands or expected counts. Store raw
artifacts and SHA-256 hashes in a fresh attempt directory and link failed attempts
from the accepted rerun. Keep the original evidence unchanged. The inherited
07 audit links each cross-repo invariant to native, socket, corpus, shipped-server,
HTTP, browser or external CTP evidence and states each layer's limits.

The 06 density pass belongs to consumer `4f129d8`; the manifest records changes
since that run. Rerun affected density/lifecycle evidence on the release candidate.
The 07 runtime records name a dirty consumer base plus source/binary hashes; the
implementation commit is separately recorded. Do not relabel that execution as a
clean-commit run. Resolve exact-commit delivery by rebuilding/rerunning affected
checks on the selected clean release commit, or explicitly retain the qualification
as an open gate. Hash equality of a few sources is not proof of the entire binary.

Complete the existing [manual review record](../.scratch/pgbuf-overlay-implementation/verification/06-shared-glyph/manual-review.md)
with actual reviewer, assistive technology/version, browser/OS, commit, settings,
case results and findings. Include lifecycle, coverage, keyboard, contrast,
reduced motion and quiet updates. Screenshots or automated assertions cannot sign
this review. Keep every required missing, skipped, failing or inconclusive result
open. Changes to code, corpus, workload or harness require an impact assessment and
reruns of affected evidence; tuning never lowers a threshold without a design
revision.

Use established local commands: `just verify` for local regression gates,
`just vite::frontend-density` for the existing density experiment, and the
[real-producer driver](runtime-overlay-verification.md) for engine journeys.
`just resource-benchmark-release` measures the disk inspector's deterministic
resource matrix; it does not implement this producer/browser workload matrix.
No hosted CI, external test execution, JIRA upload, push or PR publication is
implied by these commands or by this handoff.

## Decision compatibility

The ticket's older loopback wording is superseded by accepted
[ADR 0006](adr/0006-runtime-observations-are-loopback-web-capabilities.md): explicit
IPv4 listeners also work, with unauthenticated plain HTTP and exact Host/Origin
checks. Loopback plus SSH remains the operational example. No new transport is
introduced here. [ADR 0008](adr/0008-scope-runtime-projections-to-view-level.md)
also defines a future 64-sector Volume scope without timer rotation. Its design
is not proof that all of that implementation or its expanded density gates have
shipped. Measure the selected candidate's actual scope and keep producer scan
rotation distinct from viewport selection. This handoff edits neither decision
nor the completed planning map or parent specification.
