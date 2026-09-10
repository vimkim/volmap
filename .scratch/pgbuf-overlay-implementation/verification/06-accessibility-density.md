# Ticket 06 — Accessibility and density evidence

Status: **non-passing / acceptance remains open**. Date: 2026-09-10.
Starting consumer commit: `83058e452fac33531a6a690b04491755d54b5ccd`.

## Implemented

Sector grids now own eight semantic rows containing eight page cells each.
Arrow navigation still crosses rows. The age-bearing live-follow label no longer
announces every clock tick. Navigation moves focus to the resolved destination
heading; initial load, polling and same-route updates do not take focus.
The generated production JS/CSS have been rebuilt. Unchanged Volume sectors
retain their DOM across polling. Unadmitted pages keep explicit not-evaluated
names/counts without per-cell glyphs; revoking a viewport batch does not relabel
a verified source unavailable. LRU pending scopes use unknown colors; expired
and unavailable sources retain storage colors. Closed capture disclosures defer
their table DOM until opened, using the current capture.

The normal browser suite includes explicit quiet-update, keyboard, grid ownership,
forced-color focus and navigation assertions at Volume and Sector scales in both
browsers. Existing integrated observation cases cover state/LRU modes, exact list
indices, partial coverage, shared demand, rotation, hidden tabs, pause/resume,
restart, expiry and source failure. No producer, broker, inspection or export
contract is changed.

A separate release-build diagnostic exercises 12,288 laid-out real page cells,
normal pagination and the scripted socket producer. Browser-specific raw artifacts
record viewport-visible counts separately, keyboard-to-visible-focus samples and
paired sampled RSS. See [browser instructions](../../../web/e2e/README.md).

## Acceptance ledger

| Gate | Result | Evidence / remaining requirement |
| --- | --- | --- |
| Automated integrated accessibility | Pass in both browsers | `overlay-accessibility.spec.ts` and `observations.spec.ts`, both browser projects |
| Actual dense cells and visible update timing | Local diagnostic passes | `overlay-density.spec.ts`, 12,288 cells; 100 ms p95 limit per browser |
| Browser incremental peak RSS ≤32 MiB/tab | Local diagnostic fails; qualified peak missing | Sampled summed process RSS is diagnostic; qualified peak measurement and reference-host acceptance still required |
| Manual screen-reader review | Missing — non-passing | Reviewer, AT/version, browser/OS, date, named cases and outcome required |
| Manual visual review | Missing — non-passing | Reviewer, browser/OS, high contrast/reduced motion, both scales and lifecycle states required |
| Reference host qualification | Missing — non-passing | This shared execution host has not been designated a dedicated acceptance host |
| Exact consumer delivery evidence | Local artifacts retained; release acceptance open | Preserve committed consumer revision and raw artifacts; rerun affected evidence after code/corpus changes |

No screenshot, automated assertion or successful local timing sample substitutes
for the missing manual reviews. No thresholds have been weakened. Ticket 06 must
not be marked complete from this record; tickets 07–08 retain their prerequisites.

## Review and diagnostic history

Standards review: zero unresolved findings. The original RSS sampler could turn
read failures into zero memory; it now validates resident-page counts, traverses
the dedicated browser process tree, and fails on sampling errors. Navigation
from a scrolled map also received a regression-backed visible-focus fix.

Spec review: zero unresolved implementation findings. Timing now starts at the
validated input event timestamp, including input queue delay. Every arm retains
partial interaction/RSS samples, failure metadata, browser version and provenance.
State and LRU modes have separate thresholds; the worse per-mode p95 is reported.
Manual reviews and qualified memory/host acceptance remain outstanding requirements.

Historical diagnostics are retained in `06/first-diagnostic.tar.gz`,
`06/second-diagnostic.tar.gz`, `06/third-diagnostic.tar.gz`, `06/fourth-diagnostic.tar.gz`, and
`06/fifth-diagnostic.tar.gz`.
The first two inherited Playwright DOM tracing and are instrumented diagnostics,
not clean production performance estimates. Interrupted harness-development runs
are separately labelled. Pagination setup now tolerates the automatic sentinel
finishing a collection before the Load more button can be located. Final density
runs disable DOM tracing while preserving raw JSON evidence. The third and fourth
runs were untraced and still non-passing; do not substitute their results for a
newer consumer revision.

At `e49217a`, the worst-mode input p95 was 73.4 ms in Chromium and 58 ms in Firefox,
but sampled incremental RSS was 60.4/66.8 MiB and 59.3/67.8 MiB respectively.
These are failed memory diagnostics. The following lazy-disclosure change requires
fresh measurements. Earlier failures have not been deleted or averaged away.

## Latest exact-consumer diagnostic

Consumer: `8810d047256d70d17a8ceb204f1e3cd756062105`. Release binary and fixture-source
hashes, full host metadata and commands are in [the manifest](06/manifest.json).

| Browser | State / LRU p95 | State / LRU incremental sampled peak RSS | Result |
| --- | --- | --- | --- |
| chromium 151.0.7922.34 | 68.3 / 68.7 ms | 33.57 / 49.35 MiB | Timing diagnostic passes; sampled RSS fails |
| firefox 153.0 | 48 / 55 ms | 55.26 / 48.94 MiB | Timing diagnostic passes; sampled RSS fails |

Each browser executed four fresh one-tab arms (two enabled and two matched disabled),
40 real Tab inputs per arm: **320 inputs total**. All arms laid out **12,288 actual
page cells** through the real server and normal pagination; the initial 1920×1080
viewport contained 1,984 cells. Those are separate counts, not a claim that all
12,288 cells were simultaneously on screen. Neither browser nor mode is averaged
away. Both density tests exited nonzero because sampled RSS exceeded 32 MiB.

These results do **not** satisfy ticket 06. The local memory diagnostic fails;
qualified peak-memory evidence, a designated reference host, manual screen-reader
review and manual visual review remain missing. The current benchmark covers dense
keyboard navigation in state/LRU modes with disclosures closed; it is not a
complete release workload matrix or a measurement of an opened disclosure table.
No renderer rewrite or threshold relaxation was used to promote a result.

The implementation and automated semantic tests are reviewable; completion requires
qualified measurements that meet the unchanged limits and named manual reviews.
Further memory work should use an agreed reference host and a qualified measurement
method, rather than treating the spread between short fresh-browser runs as proof
of one specific allocation cause.

## Final verification

At consumer `8810d04`, **`just verify` passed (exit 0)**: 306 Rust tests,
73 frontend tests and 39 browser cases passed; the same three pre-existing Rust
ignores and one pre-existing Firefox skip remain. All eight new accessibility
cases executed in both browsers. Formatting, Clippy, static-musl release ELF,
types, reproducible generated assets, advisories and Cargo-only embedding passed.
See [the full log](06/final-verify.log). The separate density command **failed
both browser cases** on memory, as recorded above; it is never folded into the
passing functional-suite result. `just vite::frontend-density` provides the pinned
local entry point for subsequent diagnostic runs.

[Supporting screenshots](06/screenshots/) were retained from the final integrated
browser run. They are not manual screen-reader or visual acceptance. Older ticket
screenshots were preserved rather than overwritten by this run.

Ticket 06 remains open and non-passing. The required next evidence is qualified
peak-memory/reference-host measurement meeting 32 MiB, plus named screen-reader
and visual reviews. This record does not establish readiness for tickets 07–08.

Raw archives preserve original logs, partial samples, failure contexts and traces.
`06/final-chromium.json` and `06/final-firefox.json` expose the final summaries;
`06/final-diagnostic.tar.gz` contains their original per-arm evidence. Plain `.log`
views only remove trailing whitespace; matching `.log.gz` files preserve raw bytes.
`06/source-patches.tar.gz` preserves measured development revisions against the
starting baseline, including amendments that could otherwise be garbage-collected.
