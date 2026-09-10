# Ticket 06 render-allocation repair checkpoint

This checkpoint reduces demonstrated render allocations and fixes a transient
whole-map update when cached source metadata is unavailable. It does **not**
establish the 32 MiB memory gate. Ticket 06 remains non-passing; manual reviews
and qualified reference-host measurements also remain outstanding.

The starting point is consumer/evidence commit `a037479` and its
[allocation diagnosis](../06-allocation-profile/README.md). The repairs retain
12,288 actual rendered cells, both Chromium and Firefox, two state/LRU pairs,
40 real Tab inputs per arm, the 100 ms per-browser worst-arm p95 limit, and the
32 MiB paired incremental sampled summed process-tree RSS limit. No forced GC,
baseline padding, cell-count reduction, browser averaging, or heap/PSS substitute
is part of acceptance.

## Confirmed causes and retained changes

- Sector labels repeated the runtime projection already performed for preview
  marks. Labels now aggregate those marks. Volume lookup uses numeric page IDs
  after filtering the bounded batch to the displayed volume, avoiding a new
  composite identity string for every rendered page on each runtime update.
- Changing a sector's accessible summary reconstructed all 64 cell elements.
  A separate memoized preview now retains unchanged cell structure while its
  sector summary changes. Empty glyph elements and empty style objects are
  omitted; no static-property cache increases the disabled baseline.
- Unadmitted cells previously changed their own classes on an LRU switch.
  Their shared unknown color now comes from the Volume container's mode. Their
  native tooltip carries page identity; the sector accessible name and visible
  coverage/legend carry shared unevaluated/pending status. Captured rows and
  terminal source states retain detailed per-cell state and tooltip text.
- Cached `unavailable` capability metadata caused all cells to receive failure
  properties while a first fresh observation was still pending. The renderer
  now waits for an actual observation failure before applying that terminal
  projection. The memo comparator includes the zero/nonzero failure boundary.
  Expiry, refusal and incompatibility continue to render terminal state.
- Density setup and pagination selectors are narrowed to their controls. The
  test still performs real clicks, selection changes and keyboard input;
  separate semantic browser tests still assert accessible names. This is
  harness overhead reduction, not a claimed production allocation saving.

## Regression and allocation evidence

`pending-capture-red.tar.gz` retains a real-server browser regression with valid
normalized unavailable capability metadata and a held first observation. Before
the guard, the assertion expected zero unavailable preview cells but received
1,536. After release, the fixture returns a real failed request; a subsequent
explicit refresh uses the real producer again. The green run includes failure
and recovery, plus Volume/Sector color cases, in both browsers: six cases pass.
See `pending-capture-green.tar.gz` for exact test, log and results.

A source-mapped native-control profile after the guard attributes about 0.13 MiB
to cell construction, versus 3.83 MiB in the preceding separated-preview profile.
The remaining largest sites are mark lookup (1.66 MiB) and label aggregation
(0.91 MiB). The numeric lookup change followed that profile. The profile's
JavaScript and matching source map are retained in `guard-allocation.tar.gz`;
`guard-allocation-summary.json` contains the full source attribution. These are
sampled allocations including collected objects, **not** retained heap or RSS.

The native-control profile still failed its diagnostic RSS comparison at
32.11 MiB. Full real-input probes also failed; a reduced allocation site is not
proof that the original memory symptom is fixed.

## Native memory and rejected experiments

`memory-mappings.tar.gz` localizes growth to anonymous and shared browser
mappings; fonts account for only about 0.5 MiB Chromium / 1 MiB Firefox. Font
loading is not supported as the dominant cause. Firefox's own memory report
identifies an additional 15.63 MiB in software WebRender buffers in an earlier
candidate, alongside JavaScript object/property growth. That report was requested
only after measurement stopped, using the registered SIGRTMIN dump handler;
it did not request memory minimization. The raw reporter, exact probe and helper
are in `firefox-memory-report.tar.gz`. Signal behavior was checked against
[Mozilla's memory reporter implementation](https://raw.githubusercontent.com/mozilla-firefox/firefox/main/xpcom/base/nsMemoryInfoDumper.cpp).

Static class caches, paint containment, compositor hints/layer promotion, palette
variable overrides and moving the Volume legend to the sidebar did not establish
consistent passing results and are absent from the final candidate. The sidebar
experiment produced one passing state pair in both browsers, but that result did
not reproduce in Firefox and LRU remained over budget. No acceptance claim is
based on that run.

`experiment-results.json` indexes the raw per-browser reports from the diagnostic
archives. Patches identify intermediate production candidates. Sparse glyph
omission failed when first tried alone; it was retained later with the separated
preview because it removes redundant DOM without caching or changing glyphs.
The archive named `empty-glyph-rejected` describes that earlier isolated probe,
not a claim that all subsequent glyph omissions were reverted.

Later probes reused a persistent real fixture server and overrode both generated
JS and CSS with the candidate's exact bytes. Those archives carry byte hashes
and temporary tests. They are diagnostics, not substitutes for a fresh release
server. A JS-only override with stale CSS, two malformed regression-setup
attempts, and an unsupported about:memory UI attempt were excluded from causal
and acceptance conclusions. `remaining-debug-results.tar.gz` preserves leftover
instrumented result directories; it is not another independent acceptance run.

## Remaining work

The demonstrated full-grid JavaScript allocation is substantially reduced, but
sampled RSS still contains native rendering and allocation-lifetime effects that
these repairs do not fully explain. Next isolate LRU restyle/raster allocations
with browser-native profiles on this repaired renderer, comparing absolute enabled
peaks as well as paired increments. Do not accumulate speculative CSS changes or
raise the gate. Manual and reference-host prerequisites do not block that work.

## Verification of the retained candidate

The original `just vite::frontend-density` command was rerun with a fresh
release server, no asset overrides and both browser projects. It exits 1:

| Browser | State increment | LRU increment | Worst enabled-arm p95 |
| --- | ---: | ---: | ---: |
| Chromium | 39.05 MiB | 45.98 MiB | 63.8 ms |
| Firefox | 37.43 MiB | 20.77 MiB | 57.0 ms |

Both latency checks pass. Chromium fails both memory pairs; Firefox fails the
state pair. All arms retain 12,288 laid-out cells and 40 recorded inputs. Raw
reports, samples, host/browser/build provenance and command output are in
`release-density.tar.gz`; `final-candidate.patch` identifies the measured repair
against `a037479`. The profile candidate's generated JS hash is also recorded in
`guard-candidate-build.log`, matching `guard-allocation.tar.gz`'s source-map build.

Two-axis review found no hard standards violation and no confirmed specification
regression. Standards noted one judgement-call maintenance concern: capability
and expiry decisions now occur in the map alongside related cell projection
logic. This is recorded rather than introducing a broader presentation
abstraction during the repair. Spec review explicitly leaves memory and manual
acceptance outstanding. Neither review establishes a passing density gate.

The retained source is committed as `0a2f5ea` (`fix(web): avoid dense preview
rebuilds during observation setup`). `just verify` exits 0: Rust tests and release
checks, frontend types/artifact checks, 73 frontend unit tests and 41 browser
passes with the existing one Firefox skip. `verification.tar.gz` retains the
complete log, browser output and 12 refreshed screenshots. Historical screenshot
artifacts were restored after archiving the new captures. Screenshots are not
manual screen-reader or visual acceptance.

The subsequent exact-consumer-commit density run also exits 1. Its raw reports
identify `0a2f5ea` and are retained in `committed-density.tar.gz`:

| Browser | State increment | LRU increment | Worst enabled-arm p95 |
| --- | ---: | ---: | ---: |
| Chromium | 48.38 MiB | 51.11 MiB | 69.9 ms |
| Firefox | 25.01 MiB | 47.63 MiB | 55.0 ms |

These differing paired increments reinforce that no consistent RSS improvement
or passing memory claim follows from the allocation repair. `manifest.json`
records the exact consumer commit, generated asset hashes, raw environment and
per-browser results. Both browser cases fail overall on memory. The full evidence
set is checksummed in `sha256.json`.
