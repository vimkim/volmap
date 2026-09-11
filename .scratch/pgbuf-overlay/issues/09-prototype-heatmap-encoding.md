Type: prototype
Status: resolved
Blocked by: 05, 06

# Prototype the heatmap visual encoding

## Question

Buffer state is multi-dimensional per cell — residency × latch mode (read/write/none + waiter) × dirty × flush activity — and must compose with the mosaic's existing allocation-class color and occupancy gradient without collapsing independent dimensions. Build a throwaway UI (prototype skill; consult dataviz for color) to react to:

1. Encoding candidates — overlay tint vs border/corner glyph vs a mode toggle that swaps the mosaic's color meaning ("allocation view" / "buffer view") vs a split cell. What survives 10,000+ cells at mosaic zoom, and what survives the sector grid's 64 large cells.
2. Legend and staleness — how the legend explains the overlay, how cadence/last-observation time is shown, what a paused overlay looks like, and what "server absent" looks like (overlay simply gone, per ticket 06).
3. Latch activity is bursty and mostly invisible at poll cadence — decide whether instantaneous latch state is worth a channel at all versus dirty/residency being the primary layers, with latch shown only in the page workspace detail.

Link the prototype as an asset from this ticket; the answer records the chosen encoding, not the code.

## Comments

- 2026-09-06: Five interactive variants are captured in the throwaway worktree
  `/home/vimkim/temp/volmap-pgbuf-heatmap-encoding` on branch
  `prototype/pgbuf-heatmap-encoding`. Run `just prototype-pgbuf-heatmap` and
  switch with `?variant=A|B|C|D|E` or the floating arrow control. The artifact
  renders 10,240 cells at Volume scale and 64 cells plus selected-page runtime
  detail at Sector scale; active, stale, paused, and unavailable capability
  states are selectable.
- Automated interaction smoke-check: 10,240 Volume cells and 64 Sector cells
  rendered; URL and keyboard variant switching worked; unavailable removed all
  runtime marks; paused retained the frozen overlay; latch state remained
  detail-only.
- The user accepted every recommended choice. The resulting Variant E combines
  the composited marks from A with C's explicit LRU topology mode and removes
  animation. Primary source: commit `f1ddd33` on branch
  `prototype/pgbuf-heatmap-encoding`, file
  `web/prototypes/pgbuf-heatmap/index.html` (comparison screenshots are stored
  beside it).

## Answer

Use **composed state marks** as the default runtime overlay while preserving
the existing allocation-class color and occupancy gradient:

- A cyan inset outline means the page was resident in the adopted observation
  batch.
- An amber corner means the resident page was dirty.
- A static magenta edge means flushing was observed. Do not animate it at
  either scale; motion becomes noise in a 10,000-page mosaic.
- Keep existing finding outlines independent from these runtime marks. Color
  must not carry the meaning alone; edge placement and shape are semantic.

This encoding is identical at the Volume mosaic and 64-page Sector grid. It
survived the prototype's 160-sector/10,240-page rendering without replacing or
mixing the persistent storage colors. Reject full-cell runtime tint because it
conflates the two color systems, a storage/runtime mode switch as the sole
design because it loses simultaneous context, and paired maps because they
double the footprint and impose a visual alignment burden.

Add one explicit **LRU topology** mode beside the default state-marks layer.
That mode may use whole-cell colors for `lru1`, `lru2`, `lru3`, and `void`
because its mode label and legend make the changed color meaning explicit.
Distinguish shared and private membership structurally (the prototype uses a
dashed edge for private); show the exact kind-local list index only in selected
Page detail. Do not attempt a unique cell color for every list index.

Make runtime lifecycle state visible without changing inspection state:

- Fresh observations show their capture age.
- Stale observations remain visible but subdued and explicitly labelled.
- Pause freezes the marks and capture time while reporting that newer
  observations are available.
- When attachment is disabled, absent, refused, or incompatible, remove all
  runtime marks and show a neutral capability explanation; ordinary disk
  inspection remains unchanged.

The capability state disambiguates two otherwise identical omissions: during
an active complete scan, an omitted VPID means not resident; when the runtime
overlay is unavailable, omission means no observation source.

Latch mode, waiter presence, fix count, and exact LRU-list index remain in
selected-Page detail rather than the dense grid. Their sampled values are too
bursty or too high-cardinality to imply a stable heatmap. The prototype is the
visual primary source only; none of its throwaway code is production code.
