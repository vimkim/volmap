# Ticket 06: Chromium raster follow-up on the committed repair

Consumer `978efae` contains the [opaque-control and stable-map repair](../06-native-repair/README.md).
This follow-up changes **no production source, generated assets, acceptance
measurement or threshold**. The original full gate still fails Chromium LRU.

## Committed-source full workload

```sh
VOLMAP_DENSITY_PORT=41744 just vite::frontend-density
```

| Browser | State increment | LRU increment | Worst enabled p95 |
| --- | ---: | ---: | ---: |
| Chromium | 26.76 MiB | **41.48 MiB** | 69.2 ms |
| Firefox | 16.49 MiB | 23.48 MiB | 64.0 ms |

The command exits 1. `committed-density.tar.gz` retains the raw results and log:
both browsers, order-alternated state/LRU pairs, all 12,288 actual laid-out cells,
1,984 initially fully visible cells, 1920×1080 viewport, and 40 real Tab inputs
per arm. The 32 MiB and 100 ms thresholds, 50 ms summed-RSS sampling and absence
of forced GC are unchanged. The source is committed; unrelated user Markdown
changes remain in the worktree and its recorded dirty-diff hash. This is local
diagnostic evidence, not reference-host or qualified peak-memory acceptance.

The preceding repair measured Chromium LRU at 32.95 MiB. Its new 41.48 MiB result
means the earlier number was not a stable near-pass. No browser/mode averaging
or subtraction of unrelated-run RSS values supports an acceptance claim.

## Native phase comparison

Fresh dedicated Chromium processes use a real release fixture server on owned
port 41744. Every probe loads 12,288 cells, then records loaded, enabled/control,
LRU and post-keyboard boundaries. Physical setup controls and 40 keyboard inputs
are retained; reverse controls change only the named CSS/DOM variable. The
instrumented native categories below are not the sampled-RSS acceptance metric.

Every recorded native phase has zero resident cells. The admitted dense-view
scope therefore exercises observed-nonresident glyphs and unknown LRU backgrounds.
These probes do not establish the costs of resident outlines, dirty/flushing
marks or populated LRU topology; those require separate representative captures.

| Probe | Loaded tiles | Enabled/control tiles | LRU tiles | Post-keyboard tiles | Post-keyboard Blink |
| --- | ---: | ---: | ---: | ---: | ---: |
| Disabled | 30.00 | 30.00 | 30.00 | 38.25 | 21.13 |
| Enabled | 30.00 | 33.75 | 39.75 | 47.50 | 33.38 |
| Disabled; LRU CSS only | 30.00 | 37.50 | 37.50 | 41.25 | 22.50 |
| Enabled; pause before keyboard | 30.00 | 33.75 | 39.75 | 41.50 | 26.94 |
| Enabled; glyph paint hidden | 30.00 | 32.25 | 39.75 | 45.75 | 27.75 |
| Enabled; preview paint containment | 30.00 | 33.75 | 39.75 | 48.25 | 28.26 |
| Enabled; glyphs in flex layout | 29.75 | 33.75 | 39.75 | 47.25 | 28.25 |
| Enabled; preview layout containment | 30.00 | 33.75 | 39.75 | 46.75 | 28.40 |
| Enabled; nonresident glyph pseudo-element | 30.00 | 33.75 | 39.75 | 44.25 | 33.26 |

Values are MiB. Tiles mean `Renderer/cc/tile_memory`; Blink means
`Renderer/blink_gc`. GPU shared-image ownership aliases must not be added as
independent storage. Clock-sync markers assign memory dumps to phases;
`deterministic:false` avoids explicitly forcing collection. `native-summary.json`
also retains V8/allocator categories and DOM metadata. The reports do not identify
a retained leak or assign every RSS byte to an object type.

Three starting hypotheses were raster retention during recoloring, broader glyph
paint work, and DOM/accessibility growth during navigation. The reverse LRU CSS
control confirms recoloring cost without observation rows or React updates.
Pausing adoption before keyboard reduces both tile and Blink growth, narrowing
additional work to observation changes during navigation. Pause also revokes
the active scope, so this does not isolate polling from revocation or expiry.
Pausing the workload is never a proposed acceptance fix.

Between LRU and keyboard markers, the normal enabled trace has 605 Paint events
versus 82 disabled. Hiding glyph paint yields 88; the paused probe yields 92.
This links much of the paint-event count to glyph rendering, but hiding evidence
is not a valid repair and event counts are not allocated-byte measurements.
`keyboard-events.json` retains Paint/Layout/UpdateLayoutTree counts per probe.

The bounded alternatives did not establish a safe improvement. Paint containment
still reaches 48.25 MiB of tiles. Normal-layout glyph placement has 601 Paint
events and 47.25 MiB tiles; its loaded Blink category is higher (23.13 MiB), so
it must not be credited for inflating a baseline. Layout containment has 600
Paint events. Replacing the nonresident child glyph's paint with the same visible
CSS pseudo-element yields 1,118 Paint events and essentially unchanged Blink
growth. Its lower single tile sample does not establish a full-workload RSS
improvement, and increased Paint counts alone cannot disqualify a memory
improvement. No full RSS gate was run on these native diagnostic variants.
None was adopted or substituted into the committed-source density gate.

## Artifacts and next evidence

`native-profiles.tar.gz` retains all phase reports, compressed Chromium traces,
the final driver, helpers and logs. The driver adds diagnostic CSS only to served
responses; generated assets remain unchanged. Its final variant table reproduces
all named probes. One initial glyph command used the wrong working directory and
never launched a browser; the corrected run is the reported one. Temporary
drivers and expanded profiles were removed after archive validation; both owned
fixture-server sessions were stopped. Existing interactive servers were untouched.

To regenerate category summaries, extract the archive and run
`../06-native-allocation/summarize.py <extracted>/profiles`. To replay a probe,
copy the archived driver to `web/raster-memory.mjs`, copy the helpers to their
recorded `/tmp/volmap-raster-*.py` paths, and start the fixture server from the
repository root:

```sh
VOLMAP_BROWSER_PORT=41744 VOLMAP_BROWSER_PRODUCER=1 VOLMAP_BROWSER_DENSE=1 \
  VOLMAP_BROWSER_RELEASE=1 release/run-browser-server.sh
```

Then run sequentially from `web/`:

```sh
mise x node@24.19.0 -- node raster-memory.mjs chromium enabled base
mise x node@24.19.0 -- node raster-memory.mjs chromium disabled recolor-only
```

Use a separate extraction/checkout to avoid replacing committed evidence.
Further repair needs a glyph-rendering or observation-update change that preserves
all marks, freshness, revocation and bounded admission while reducing the original
full-workload peak. The tested variants did not establish an improvement;
they do not establish that the 32 MiB gate is impossible. Reference-host
designation and named screen-reader/visual reviews have been requested separately.
Ticket 06 and its existing human/measurement prerequisites remain open.

Read-only standards review found no documented breaches. Spec/evidence review
verified the table, density counts and selected raw trace Paint counts, and
requested the zero-resident workload caveat and narrower single-sample conclusion
above. No production changes require repeating the preceding repair's passing
`just verify`; this turn reruns the original density gate and validates archives.
