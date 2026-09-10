# Ticket 06 allocation investigation

The dense overlay has two demonstrated allocation contributors: automation
selector work and application rendering during enable/mode changes. Removing
selector work from the diagnostic does not eliminate the RSS failure. The
application profiles identify per-cell JSX/property construction, sector labels,
storage class strings and React reconciliation as the principal allocation
sites. They do not establish a retained-memory leak or attribute every RSS byte.

No production code, acceptance threshold or acceptance workload changed in this
investigation. Ticket 06 remains non-passing. This is an allocation diagnosis,
not completion of the implementation or its manual review requirements.

## Feedback loop and current failures

At checkout `ed5f225` (production consumer `8810d04`), the unchanged release
browser command failed independently in both browsers:

```sh
mise x node@24.19.0 -- corepack pnpm --dir web exec playwright test --config playwright.density.config.ts --project=chromium
mise x node@24.19.0 -- corepack pnpm --dir web exec playwright test --config playwright.density.config.ts --project=firefox
```

| Browser | State incremental sampled RSS | LRU incremental sampled RSS | Worst p95 input latency |
| --- | ---: | ---: | ---: |
| Chromium | 40.40 MiB | 48.50 MiB | 73.1 ms |
| Firefox | 56.48 MiB | 60.20 MiB | 57.0 ms |

Each command exited 1 on the unchanged 32 MiB memory assertion. Each arm rendered
12,288 cells and used 40 real Tab inputs. `baseline.tar.gz` and
`baseline-firefox.tar.gz` retain reports, original logs, browser versions, host,
corpus hash and raw samples. These are local sampled process-tree RSS results;
no new qualified peak-memory claim is made.

The archived Chromium one-LRU-pair loop reproduced with 40 inputs, then with
one input (74.06 MiB increment). Further cell-count reduction was deliberately
not used: the user symptom explicitly includes the dense workload. This narrows
the interaction trigger, not the acceptance scope. Instrumented profiling runs
are diagnostic comparisons, not pass/fail replacements.

## Predictions and results

The ranked predictions were dense React updates, observation decoding/polling,
browser-native allocations, and automation overhead. The tests distinguish them
as follows:

1. Dense updates: source-mapped allocation profiles consistently concentrate in
   cell construction and related render paths. Splitting native-control setup
   records 17.69 MiB allocated while enabling and 7.69 MiB while changing to LRU;
   the following Tab interaction allocates 0.10 MiB. This supports setup/render
   allocation as a target rather than prolonged keyboard churn.
2. Decoding/polling: neither is among the dominant allocation stacks in this
   bounded setup reproduction. This does not rule out other long-running
   workloads; it gives no evidence for changing the broker or batch limits.
3. Native memory: process snapshots put most growth in the renderer. In the CSS
   process probe, renderer RSS grows from 228.95 to 267.68 MiB during setup;
   GPU-process RSS grows from 92.02 to 99.52 MiB. Browser/utility/zygote growth is
   small. Renderer anonymous memory grows by about 23.71 MiB. This localizes RSS,
   but does not split native style/layout allocations from V8 heap reservations.
4. Automation: controlled replacement of role selectors with CSS selectors,
   then native DOM controls, removes their allocation stacks. Native controls
   still produce a 43.63 MiB incremental sampled RSS failure. Thus automation
   contributes to the measured cost but is not a sufficient explanation.

| Diagnostic setup | Sampled allocated bytes during enabled setup | Selector attribution |
| --- | ---: | --- |
| Role selectors | 44.83 MiB | 19.23 MiB under `queryRole` |
| CSS selectors | 30.68 MiB | 4.74 MiB under `querySelectorAll` |
| Native DOM controls | 24.80 MiB | No dominant selector stack |

These are sampled allocation estimates including subsequently collected objects,
not live retained heap or RSS. Do not subtract them from acceptance RSS. Runs use
separate browser processes and are not precise estimates of small differences.
Native controls invoke the real event handlers but do not substitute for physical
user interactions in acceptance tests. All production controls and tests remain
unchanged.

## Source attribution

The source-map build produced JavaScript byte-for-byte identical to the committed
and served bundle (SHA-256
`0cc5c1ff87f0ab21ce4e178b076a364618126f61861d4513e2fb444b554a58fd`).
The generated map resolves the native-control profile to these sites:

| Site at consumer `8810d04` | Sampled allocation attribution | Mechanism visible in source |
| --- | ---: | --- |
| `web/src/view.tsx:173`, page callback | 8.57 MiB | Recreates cell JSX, properties, title and child glyph element during a sector render |
| `web/src/observation-view.tsx:170`, `sectorObservationLabel` | 3.24 MiB | Reprojects page marks and aggregates label counts for each sector |
| `web/src/view.tsx:24`, `pageClass` | 2.57 MiB | Rebuilds storage class strings although disk facts did not change |
| React JSX runtime | 2.14 MiB | Creates element records |
| React fiber constructor | 1.57 MiB | Creates reconciliation work records |
| `web/src/view.tsx:125`, marks projection | 1.31 MiB | Rebuilds mark arrays independently of sector-label projection |

Values attribute native builtins to the nearest mapped script caller and should
be read with the full stack summaries. The summarizer emits top sites and stacks;
raw CDP profiles retain the complete trees. Frame entry locations are callsite
attribution, not a claim that every byte was allocated on that exact line.

The source shows why a bounded observation request can still allocate across a
dense volume: enabling observations changes each sector's observation label and
marks; LRU mode changes unknown-cell classes as well as admitted cells. The
sector memo comparator must then admit those renders. Each admitted sector
render reconstructs its 64 cell elements and static disk properties. Existing
sector memoization avoids unchanged-sector work during later batch rotation,
but cannot eliminate these initial global transitions.

## Next bounded repair targets

Use the existing real-server density test as the regression seam. In order:

1. Reuse immutable storage presentation properties across runtime-only renders,
   and derive sector label counts from the already-computed marks rather than
   calling `runtimePage` again. Check absolute enabled RSS as well as incremental
   RSS, so increasing disabled baseline is never mistaken for an improvement.
2. Measure each change separately against the same profile and full acceptance
   test. If element/reconciliation allocations still dominate, investigate a
   component boundary that reuses static cell structure without adding a costly
   component per cell or changing unknown/expired/non-color semantics.
3. Separately reduce acceptance-harness selector overhead while retaining real
   user actions and separate accessibility assertions. Do not credit that work
   as a production memory fix or promote one diagnostic native-control run.

Neither source inspection nor the measured allocation sites prove that these
changes will meet 32 MiB. They provide specific falsifiable repair targets.
Retain both-browser timing/memory gates, lifecycle invariants, manual reviews and
reference-host qualification. There is no evidence here justifying a request
budget change, forced GC in production, a renderer rewrite or threshold relief.

## Reproduction and cleanup

Profiles used Chromium CDP `HeapProfiler.startSampling` with 16 KiB sampling and
collection-inclusive options. Additional probes record performance/DOM counters
and `/proc` process RSS plus `smaps_rollup`. Instrumentation changes timing and
memory; its outcomes are diagnostic only.

Each profile archive contains its exact temporary test and configuration, raw
results and original run log. Restore the files as
`web/e2e/memory-probe.spec.ts` and `web/playwright.memory-probe.config.ts` and run:

```sh
mise x node@24.19.0 -- corepack pnpm --dir web exec playwright test --config playwright.memory-probe.config.ts
```

The two minimization archives retain the test; use the same configuration from
`allocation-role-selectors.tar.gz`. The test title/report method inherited from
the acceptance harness is historical; source and raw interaction counts identify
the actual one-pair/one-input probe. These reduced runs are not acceptance.

`matching-source-map.tar.gz` retains the matching bundle, source map and build
log. It was produced outside the repo's generated output:

```sh
VOLMAP_FRONTEND_OUT_DIR=/tmp/volmap-profile-sourcemaps mise x node@24.19.0 -- corepack pnpm --dir web exec vite build --sourcemap hidden
```

Use `summarize-alloc.py SOURCE_MAP EXTRACTED_RESULTS OUTPUT_JSON` to reconstruct
the checked-in allocation summaries. `sha256.json` records artifact integrity.
Temporary tests/configuration were removed from the active suite. No profiler,
debug logging, source map or forced GC was added to the product. The original
reproduction still fails; this report deliberately stops at the requested
allocation investigation and does not claim the skill's fix/green phase.
