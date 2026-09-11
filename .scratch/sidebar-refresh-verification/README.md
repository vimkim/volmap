# Stable sidebar refresh and letter-first LRU labels

The user reported sidebar flicker on the live Volume view and requested `S1` /
`P3` instead of `1S` / `3P`.

Before the fix, automatic observation requests replaced retained status text
with shorter loading messages in the controls, legend and observation summary.
The volume navigation moved between y=1512.75 and y=1459.5625 at a 1600x1000
viewport, a 53.1875-pixel shift. The refresh button also alternated disabled
colors and outline on each request.

A retained capture now keeps its status during refresh. The refresh button is
still disabled during the request, reports its busy state, and preserves its
appearance. Initial loading, paused/unavailable controls, errors and expiry
remain explicitly displayed. Requests continue at the existing cadence.
LRU membership letters precede zones, retaining dirty/flushing suffixes and
leaving unknown membership unspecified.

Verification: the new reducer assertion and changed glyph assertion failed
before the fix (`unit-red.log`). Types and all 82 frontend tests passed;
all 56 observation browser tests passed in Chromium and Firefox, including
pause, expiry, errors, forced colors and the pending-refresh geometry/style
regression. Generated JS/CSS were rebuilt and the debug viewer was restarted.

The live Volume 1 check (`live.json`) sampled 60 times across three captures:
one sidebar position, one status, one button appearance, both busy states,
and visible `S1`/`P3` labels. [Live screenshot](live.png).
