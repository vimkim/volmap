# 01: Build the Volume occupancy mosaic

**What to build:** Introduce the focused TUI session and a non-production Volume renderer that shows a scrollable mosaic of Sector cards. Follow F1 and the occupancy vocabulary in the [focused TUI implementation specification](../implementation-spec.md). Every visible card must contain all 64 physical Pages, with allocation and occupancy encoded independently through the accepted two-column microcell and eight-level Braille/ASCII profiles.

**Blocked by:** [Establish the React compatibility viewer](../../volmap-live-web-runtime/issues/02-establish-react-compatibility-viewer.md).

**Status:** ready-for-agent

- [ ] The session owns an immutable current `GraphView`, exact Volume identity, focused Sector, top Sector anchor, and semantic actions without importing HTTP, React, or legacy tab state.
- [ ] Each visible Sector card presents all 64 Pages in physical 8×8 order and preserves allocation, known zero, known positive occupancy, unknown occupancy, not-applicable occupancy, finding presence, and file/class/table attribution as distinct facts.
- [ ] Positive occupancy uses `ceil(percent × 8 / 100)` with the specified eight Braille levels; monochrome/ASCII fallback uses the same buckets and never collapses `0`, `?`, or `-`.
- [ ] Arrow movement follows the installed card grid, scrolling reveals every Sector without sampling, and `[`/`]` plus `PageUp`/`PageDown` retain their bounded sibling semantics.
- [ ] Projection and retained presentation data are limited to visible complete card rows plus one row of overscan and do not grow with the Volume's total Sector count.
- [ ] Source-derived labels use one control-sanitizing, grapheme-safe, display-column-aware text path before placement.
- [ ] Deterministic session traces and Volume goldens cover 120×36 ANSI/Unicode, 80×24 ANSI/Unicode, and 60×20 monochrome/ASCII without switching production `tui::run` yet.
