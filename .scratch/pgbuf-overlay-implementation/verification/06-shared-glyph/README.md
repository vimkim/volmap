# Ticket 06: shared nonresident circle

This repair follows the [Chromium raster follow-up](../06-raster-followup/README.md).
It replaces the repeated Volume nonresident text box with a shared SVG background
circle. Accessible sector summaries, page titles, observation tables, and all
resident/unknown/ambiguity glyphs retain their existing rendering. Forced colors
use a system-colored `○` pseudo-element instead of the background image.

The marker is one background layer above the original disk occupancy fill.
Known and unknown occupancy retain their existing gradients and sizing; LRU
nonresidency replaces only that fill with unknown gray. Disabling or revoking
the mark restores the ordinary storage projection. Sector and selected-page
rendering remain unchanged.

## Allocation evidence before implementation

The native CSS-response probe retains all nonresident marks but draws the circle
as a shared image while suppressing the old child glyph's layout. Chromium's
post-keyboard Blink category is 24.50 MiB, compared with 33.38 MiB in the preceding
normal probe; tiles remain similar (47.25 versus 47.50 MiB). These are instrumented
category samples, not exact per-object attribution or the RSS acceptance metric.
As in that preceding probe, resident count is zero: this targets nonresident
glyphs, not resident/dirty/flushing rendering. `native-profile.tar.gz` retains
the driver, helper scripts, phase metadata and compressed trace; the expanded
profile and temporary driver were removed after byte comparison with the archive.

The full CSS-response diagnostic preserved both browsers, both state/LRU pairs,
12,288 laid-out cells and all 40 keyboard inputs per arm:

| Browser | State increment | LRU increment | Worst enabled p95 |
| --- | ---: | ---: | ---: |
| Chromium | 24.72 MiB | 28.25 MiB | 49.6 ms |
| Firefox | 28.93 MiB | 29.69 MiB | 55.0 ms |

It exits 0, but is **not production acceptance**: CSS is fulfilled by Playwright,
which also changes browser caching behavior, and the simple rule had not yet
handled allocated-page occupancy. `diagnostic-density.tar.gz` includes the exact
temporary test/config, CSS, reports and log. No threshold, rendered count, browser,
input count or forced-GC setting was changed. Temporary test/config files were
removed before production verification.

## Regression and review

The nonresident marker regression was red in both browsers before implementation
(`background-image: none`). After the change it checks shared-image rendering,
absence of a child glyph box, retained LRU mark, forced-color glyph, stable cell
geometry, legend meaning and clearing on disable. Normal and forced-color sector
screenshots are retained here for both browsers; they are not named human reviews.

Both review axes caught the first candidate's conflict with allocated-page
occupancy gradients. A second regression varies known (35% occupied) and unknown
occupancy through the real HTTP decoder, then captures the original storage fill.
It was red for missing circle layers in both browsers. An intermediate layered
version exposed transparent LRU fill; that failure was also retained. The final
composition preserves the original gradient layer, supplies LRU gray explicitly,
and restores the original background image and size on disable. Both reviewers
confirmed their finding resolved with no remaining confirmed defects.

The final focused command passes 12 cases across Chromium and Firefox, covering
the two new regressions plus existing state/LRU, resident marks, partial unknowns,
ambiguity, nonresidency, expiry and forced-color behavior:

```sh
mise x node@24.19.0 -- corepack pnpm --dir web exec playwright test observations.spec.ts \
  --grep 'share an image|preserve known and unknown occupancy|composes state marks|separates partial unknowns'
```

`regression-red.tar.gz`, `regression-green.tar.gz`, `occupancy-red.tar.gz`,
`occupancy-lru-red.tar.gz` and `final-regression-green.tar.gz` preserve this history.
The earlier unlayered production candidate's RSS run is retained separately in
`unlayered-density-rejected.tar.gz` (Chromium 23.88/33.83 MiB, Firefox
20.41/31.91 MiB). It failed memory overall and had the occupancy regression;
it cannot support acceptance of the final implementation.

## Original production density gate

```sh
VOLMAP_DENSITY_PORT=41744 just vite::frontend-density
```

The final layered implementation passes both browser cases through embedded
production assets, with no CSS override or measurement change:

| Browser | State increment | LRU increment | Worst enabled p95 |
| --- | ---: | ---: | ---: |
| Chromium | 14.24 MiB | 30.96 MiB | 66.4 ms |
| Firefox | 8.12 MiB | 17.29 MiB | 93.0 ms |

`production-density.tar.gz` retains the exact implementation, original test's
reports and command log. Every arm retains 12,288 laid-out cells, 1,984 initially
fully visible cells, 1920×1080 viewport, and all 40 real Tab inputs. Both order-
alternated state/LRU pairs, 50 ms summed-RSS sampling, the 32 MiB ceiling and
100 ms timing limit remain unchanged. No forced GC or inflated disabled workload
is introduced. This is checkout `9ebced2` plus the recorded repair source;
an exact-commit rerun must follow the source commit.

This is a passing **local sampled diagnostic**, not proof of a designated
reference host, qualified peak measurement, or named manual screen-reader and
visual reviews. Those requirements remain outstanding. No failed historical
result is replaced by this pass, and one run does not establish statistical
repeatability across hosts or browser versions.

`just verify` passes on the final source: Rust, Clippy, static-musl release and
generated-asset checks, 73 frontend unit tests and 47 browser passes with the
existing one Firefox skip. `verify.tar.gz` retains the full log, test results and
updated integration screenshots. Historical screenshot paths were restored after
archiving new captures. `source-manifest.json` identifies all six changed source,
test and generated files; `repair.patch.gz` retains their exact delta against
`9ebced2`. Both standards and spec reviewers confirmed the occupancy corrections
and reported no remaining confirmed implementation findings.
