Type: prototype
Status: open
Blocked by: 05, 06

# Prototype the heatmap visual encoding

## Question

Buffer state is multi-dimensional per cell — residency × latch mode (read/write/none + waiter) × dirty × flush activity — and must compose with the mosaic's existing allocation-class color and occupancy gradient without collapsing independent dimensions. Build a throwaway UI (prototype skill; consult dataviz for color) to react to:

1. Encoding candidates — overlay tint vs border/corner glyph vs a mode toggle that swaps the mosaic's color meaning ("allocation view" / "buffer view") vs a split cell. What survives 10,000+ cells at mosaic zoom, and what survives the sector grid's 64 large cells.
2. Legend and staleness — how the legend explains the overlay, how cadence/last-observation time is shown, what a paused overlay looks like, and what "server absent" looks like (overlay simply gone, per ticket 06).
3. Latch activity is bursty and mostly invisible at poll cadence — decide whether instantaneous latch state is worth a channel at all versus dirty/residency being the primary layers, with latch shown only in the page workspace detail.

Link the prototype as an asset from this ticket; the answer records the chosen encoding, not the code.

## Comments

## Answer
