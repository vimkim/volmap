# Ticket 06: native allocation causes after the render repair

Two native rendering mechanisms are now isolated on the repaired consumer
`0a2f5ea` (checkout `2a2c0e3`). Firefox allocates additional software WebRender
storage when the pending refresh button uses group opacity. Chromium allocates
additional raster tiles when the dense map moves or changes color. These are
separate from the previously repaired React construction costs.

**No production code, generated asset, acceptance harness, threshold or browser
scope changed in this investigation. Ticket 06 remains non-passing.** The
opacity-only diagnostic passes Firefox's full memory pairs, but Chromium still
fails. This is cause attribution and a repair target, not acceptance.

## Red feedback loop

Starting from [the committed repair evidence](../06-memory-fix/README.md), reran:

```sh
just vite::frontend-density
```

It exits 1, with both browser cases failing the 32 MiB memory assertion:

| Browser | State increment | LRU increment | Worst enabled-arm p95 |
| --- | ---: | ---: | ---: |
| Chromium | 48.46 MiB | 43.35 MiB | 65.2 ms |
| Firefox | 42.73 MiB | 35.84 MiB | 56.0 ms |

`baseline.tar.gz` retains the reports and log, including exact commit, browser
versions, host, corpus hash, samples and all 40 real Tab inputs per arm. Every
arm lays out 12,288 cells. The 32 MiB gate and 100 ms timing limit are unchanged;
results are never averaged across browsers or modes.

A separate copy reduced only the keyboard sequence to one Tab input. It still
exits 1: Chromium 102.49/49.96 MiB, Firefox 53.94/41.44 MiB. The variable sampled
RSS values do not support fine-grained subtraction between runs, but the failure
survives without prolonged keyboard activity. `one-input.tar.gz` contains the
exact reduced test/config and raw results. This reduction is diagnostic only.
Further controls below isolate painting triggers without adopting observations;
they do not reduce or replace the dense acceptance case.

## Firefox: pending-button opacity allocates software rendering storage

The production rule is `.observation-controls button:disabled { opacity: 0.55; }`
in `web/src/observations.css`. A refresh button becomes disabled while an
observation request is pending. Native reports retain the resulting software
WebRender allocation after that pending state finishes.

| Native-control probe | Loaded SWGL storage | After enable/control | After LRU/control |
| --- | ---: | ---: | ---: |
| Normal overlay | 99.05 MiB | 114.39 MiB | 114.39 MiB |
| Only disabled-button opacity forced to 1 | 99.05 MiB | 98.38 MiB | 98.38 MiB |
| Observations disabled; existing enable button given opacity 0.55 | 99.05 MiB | 114.06 MiB | 114.06 MiB |

The override removes **16 MiB** from the post-enable SWGL category in this
comparison. The reverse control recreates about 15 MiB of growth with zero
observation glyphs or resident marks. That supports opacity as a cause rather
than an incidental correlation with enabling the overlay. It does not mean
every translucent button allocates a separate 16 MiB: the report identifies
browser rendering storage, not ownership by individual DOM elements.

The category is Firefox's `explicit/gfx/webrender/swgl`, summed across reported
processes. These are native reporter bytes, not the acceptance RSS metric. No
memory minimization or forced garbage collection was requested.

Other probes narrowed the cause:

- Hiding the legend's paint while retaining its space did not remove the growth.
- Removing the legend from layout did not remove it in Firefox either.
- Hiding map glyphs alone did not remove it.
- Disabling sidebar scrolling did not remove it.
- Hiding the sidebar removed the additional SWGL growth; hiding its glyphs too
  did not produce a corresponding further SWGL reduction.
- Repainting only the map background with observations disabled did not trigger
  the same allocation. Thus this is not simply the cost of any repaint.

The hidden-sidebar group uses native DOM control handlers because hidden
controls correctly reject physical clicks. Its baseline was rerun with the same
native handlers. The actual acceptance and full-workload diagnostic below use
physical Playwright actions. An earlier hidden-control timeout is excluded.

## Chromium: displacement and recoloring allocate raster tiles

Chromium tracing exposes renderer `cc/tile_memory` and GPU `gpu/shared_images`.
The same storage is represented through allocator ownership relationships; these
paths must **not** be added together as independent allocations. The unchanged
acceptance metric intentionally sums process RSS, which can count shared mappings
in more than one process.

| Probe | Loaded tile storage | After enable/control | After LRU/control |
| --- | ---: | ---: | ---: |
| Normal overlay | 30.00 MiB | 42.30 MiB | 42.30 MiB |
| Opacity-only override | 30.00 MiB | 42.25 MiB | 42.25 MiB |
| Legend removed from layout | 30.00 MiB | 34.05 MiB | 40.05 MiB |
| Observations disabled; map shifted down 125.2 px | 30.00 MiB | 40.25 MiB | 40.25 MiB |
| Observations disabled; existing LRU CSS mode applied to map | 30.00 MiB | 37.50 MiB | 37.50 MiB |

The reverse displacement control recreates **10.25 MiB** of tile growth with
V8 size unchanged at 33.29 MiB at that boundary. The recolor-only control adds
**7.50 MiB** of tiles with V8 unchanged at 33.54 MiB. Blink GC size increases by
only about 0.19 MiB for displacement and 1.62 MiB for recoloring. This isolates
substantial native raster allocation without React updates or observation rows.

The legend moves the real map from about y=196 to y=321.2. Its removal reduces
enable-time tile growth, but subsequent LRU and keyboard phases still grow tile
storage. That explains why previous sidebar-legend experiments did not establish
a stable whole-workload RSS pass. The opacity override does not remove Chromium's
tile growth and must not be presented as a both-browser solution.

The normal overlay's later keyboard phase also grows Blink/allocator categories
and tile storage. These results do not attribute every RSS byte, demonstrate a
retained leak, or prove a complete repair. The layer-tree probe did not expose a
new sidebar layer; no conclusion relies on such a layer existing.

## Check against the full workload

A separate diagnostic appended only the opacity override to the served CSS,
leaving generated assets untouched. It retained both browser projects, both
state/LRU pairs, 12,288 laid-out cells and all 40 real keyboard inputs per arm.
The command exits 1: one browser case passes and one fails.

| Browser | State increment | LRU increment | Worst enabled-arm p95 |
| --- | ---: | ---: | ---: |
| Chromium | 45.54 MiB | 38.44 MiB | 58.3 ms |
| Firefox | 11.52 MiB | 12.33 MiB | 67.0 ms |

`full-opacity.tar.gz` retains the exact diagnostic test/config, CSS hash and raw
results. This supports the Firefox attribution in the original workload. It is
not a production acceptance run: the override removes an existing visual disabled
cue, has not been replaced with a reviewed accessible treatment, and still fails
Chromium. Manual reviews and reference-host qualification remain outstanding.

## Instrumentation and reproducibility

`native-profiles.tar.gz` contains phase reports, compressed Chromium traces,
Firefox reports, logs, the final probe driver and helpers. Phases are loaded,
enabled, LRU, and after 40 keyboard inputs. A real release fixture server serves
the committed renderer. Each probe owns a fresh browser tree/tab and changes one
variable relative to its stated control. Native profiles are instrumented
comparisons, never replacements for the uninstrumented sampled RSS gate.

Chromium uses `Tracing.requestMemoryDump` with `deterministic: false` and detailed
memory reporting. The pinned Playwright protocol declaration documents that
`deterministic: true` forces GC; it is not used. Trace-local dump IDs differ from
returned CDP GUIDs, so authoritative phase attribution uses explicit `clock_sync`
markers and event timestamps. Initial unmarked traces are retained but excluded
from the phase summary. Summaries retain named allocator paths rather than
adding parent/child or ownership aliases.

Firefox uses its registered SIGRTMIN report handler on the dedicated browser PID,
checking the registered handler first. It does not use the memory-minimization
signal. Reports perturb timing and may allocate reporter bookkeeping; the
matched disabled and reverse controls are needed for interpretation. Phase
metadata records source text, mode, map position and, in later probes, resident
and glyph counts. Visibility changes may alter admitted identities; the reverse
controls use no observations and avoid relying on that comparison.

The driver evolved during diagnosis: initial probes use physical setup controls;
the hidden-sidebar group and later controls use native handlers, recorded as
`controlMode` in the summary. The archived final driver reproduces the latter
controls. Two shell path mistakes and the hidden-control timeout are excluded;
no result is attributed to a variant that did not execute.

To recompute the summary from the retained raw files:

```sh
mkdir -p /tmp/volmap-native-evidence
# Run from this evidence directory.
tar -xzf native-profiles.tar.gz -C /tmp/volmap-native-evidence
python3 summarize.py /tmp/volmap-native-evidence/profiles
```

The output is `native-summary.json` inside the extracted profiles directory.
To run the archived driver, copy its `web/native-memory.mjs` into the checkout's
`web/`, copy both `helpers/*.py` into `/tmp/`, start the real server with
`VOLMAP_BROWSER_PORT=41742 VOLMAP_BROWSER_PRODUCER=1 VOLMAP_BROWSER_DENSE=1
VOLMAP_BROWSER_RELEASE=1 release/run-browser-server.sh`, then run from `web/`:

```sh
mise x node@24.19.0 -- node native-memory.mjs firefox enabled opaque-disabled
mise x node@24.19.0 -- node native-memory.mjs firefox disabled opacity-only
mise x node@24.19.0 -- node native-memory.mjs chromium disabled shift-only
mise x node@24.19.0 -- node native-memory.mjs chromium disabled recolor-only
```

Run them sequentially. Output goes under this evidence directory; use a separate
checkout/extraction to preserve the committed evidence. Stop the owned fixture
server and remove temporary drivers afterward. All probe drivers were removed
from the normal test/source tree at the end of this investigation.

## Next bounded repair

1. Replace pending-button group opacity with an opaque disabled appearance while
   preserving native disabled behavior, keyboard behavior and visible distinction.
   Lock down those semantics before the CSS change, then rerun both full browser
   gates. The diagnostic override is not the finished accessible design.
2. Separately avoid inserting the legend into the dense map's vertical flow while
   keeping it visible and accessible. Measure tile categories and the complete
   state/LRU/keyboard workload; do not credit an enable-only reduction as a pass.
3. If Chromium remains above 32 MiB, investigate tile allocation/reuse across the
   unavoidable LRU recolor and keyboard scrolling on that repaired layout. The
   controls here identify raster allocation as a target; they do not justify
   shrinking rendered cells, hiding evidence, padding the disabled baseline,
   forcing GC, or raising the ceiling.

No ticket checkbox is closed by this diagnosis. The newly isolated causes allow
bounded repair to proceed independently of outstanding human acceptance inputs.
