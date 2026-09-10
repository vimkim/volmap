# Ticket 06: opaque controls and stable Volume map

This repair follows the [committed native allocation diagnosis](../06-native-allocation/README.md).
It removes the demonstrated Firefox pending-button opacity trigger and the
Volume legend's displacement of the dense map. **Ticket 06 remains non-passing:**
the full local run still exceeds 32 MiB in Chromium LRU mode. Manual accessibility
and visual reviews and reference-host/peak-memory qualification remain missing.

## Changes and regression evidence

The pending refresh button now uses opaque panel/muted colors and an inset dashed
outline. It retains native disabled behavior and uses Canvas/GrayText in forced
colors. The regression holds a real observation request pending and checks the
disabled state, opacity and outline. Both browsers failed before the CSS change
(opacity 0.55 instead of 1), then passed after it.

The Volume legend now appears beside the observation controls in the scrollable
sidebar. Its named region, mode explanation, age, source and limitations remain
available. The Sector legend stays in its existing location. The new regression
measures the map's document position before enable, after enable and after LRU
selection, and checks the legend text in both modes. Before relocation, enabling
moved the map down about 142 pixels in both browsers; afterward its position
remains unchanged. The two focused cases pass in both browsers (4 passes).

Archives retain the red and green browser reports, exact tests and logs:
`disabled-appearance-red.tar.gz`, `disabled-appearance-green.tar.gz`,
`map-position-red.tar.gz`, and `legend-green.tar.gz`.

## Full original workload

```sh
VOLMAP_DENSITY_PORT=41744 just vite::frontend-density
```

Port 41742 was occupied by an existing server. The optional validated port allows
a fresh owned release server without stopping that process. The default remains
41742; server reuse remains disabled and the actual URL enters raw provenance.
No measurement logic or threshold changes. Both browsers retain both order-alternated
state/LRU pairs, 12,288 laid-out cells, 1,984 initially fully visible cells,
1920×1080 viewport and all 40 real Tab inputs in every arm. Summed browser-tree
RSS is sampled every 50 ms from launch; no forced GC is used.

| Production variant | Browser | State increment | LRU increment | Worst enabled p95 |
| --- | --- | ---: | ---: | ---: |
| Opaque disabled appearance only | Chromium | 43.32 MiB | 43.96 MiB | 61.2 ms |
| Opaque disabled appearance only | Firefox | 17.97 MiB | 19.62 MiB | 63.0 ms |
| Plus Volume legend relocation | Chromium | 29.62 MiB | **32.95 MiB** | 53.4 ms |
| Plus Volume legend relocation | Firefox | 23.73 MiB | 23.45 MiB | 58.0 ms |

Both commands exit 1 overall. `opaque-density.tar.gz` and `legend-density.tar.gz`
retain raw samples and logs. The latter also retains exact repair source and
test/config files. Measurements use checkout `3b8bdce` plus the recorded repair
diff; they are not clean-commit acceptance evidence. Different-run RSS deltas
must not be subtracted as precise causal byte attribution. The controlled native
profiles in the preceding diagnosis establish the causes; these full runs test
the repairs against the original workload and show the remaining failure.

## Review and remaining work

Standards review: no documented breaches or substantial new code smells.
Spec review: no confirmed incorrect implementation or scope creep; memory and
human/reference-host acceptance remain incomplete. Reviewers inspected the scoped
worktree diff against `3b8bdce`, including the new density-address helper.

The first `just verify` run passed Rust, Clippy, static-musl and frontend unit
checks but had one Firefox cross-tab navigation timeout (42 browser passes,
one existing skip). The trace shows `selected.goto(.../page/0/10)` stalled before
commit for about 29 seconds after the preceding coverage assertions passed.
`verify-initial.tar.gz` preserves the failure, trace, log and rendered screenshots.
The unchanged focused case then passed in 5.1 seconds; `cadence-recheck.tar.gz`
retains that result. No timeout, retry, test or production change was made in
response to this isolated failure. A focused pass alone does not prove its cause.

The subsequent unchanged `just verify` completed successfully: Rust and release
checks, 73 frontend unit tests, and 43 browser passes with the existing one Firefox
skip. `verify-green.tar.gz` preserves its full log, browser results and current
screenshots. Historical screenshot paths were restored after archiving the new
captures. `source-manifest.json` identifies all eight changed source/artifact
files by SHA-256; `repair.patch.gz` retains the tracked source delta. The new
density-address helper is also retained in `legend-density.tar.gz`.

Continue from this repaired layout with native Chromium profiles across LRU
recolor and keyboard scrolling. A passing state pair or Firefox result does not
establish both-browser acceptance. Preserve the 32 MiB and 100 ms ceilings,
rendered density, complete interaction workload, and all runtime disclosure
and accessibility behavior. Do not compensate by padding the disabled baseline,
hiding cells or evidence, forcing GC, or averaging modes/browsers.
