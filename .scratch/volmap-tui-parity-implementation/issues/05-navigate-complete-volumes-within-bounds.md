# 05: Navigate complete Volumes within bounded resources

**What to build:** Make the Atlas Volume mosaic completely navigable at small and maximum topology sizes without sampling, unbounded state, or viewport-dependent identity. Apply the Volume portion of [Set volume viewport and rendering resource budgets](../../volmap-tui-web-parity/issues/07-set-viewport-resource-budgets.md).

**Blocked by:** 04: Present the first Atlas Volume screen.

**Status:** superseded

**Superseded by:** [Volmap focused TUI implementation specification](../../volmap-tui-focused-inspector/implementation-spec.md).

- [ ] Atlas retains one contiguous exact-revision reservoir of at most 64 complete Sectors and requests only bounded, gap-free windows from the Projection workspace.
- [ ] The fixed complete Sector-card geometry, packing stride, capped logical canvas, complete visible rows, and one-row overscan match the accepted viewport contract at every supported tier.
- [ ] Arrow focus, wheel scrolling, page movement, direct reveal, and first/middle/last jumps can reach every Sector in physical order without walking or materializing total Volume size.
- [ ] Keyboard and mouse actions converge on the same semantic transitions; the wheel routes to the hovered region and stale-generation pointer input is ignored.
- [ ] Resize preserves exact revision, focused Sector, and semantic top anchor across wide, compact, too-small, and oversized physical surfaces; new hit regions activate only after flush.
- [ ] Nearby movement reuses overlap, distant movement replaces atomically, and query failure preserves the last complete scene and focus without partial installation.
- [ ] Lazy maximum-topology and exhaustive 257-Sector tests prove no sampling, no arithmetic overflow, complete 64-Page cards, constant resident cardinality, and bounded projection work.
- [ ] Volume-specific cell, row, reservoir, redraw, and memory accounting stays inside the accepted limits without altering Inspection coverage or outcome.
