# 06: Drill into a complete Sector and restore focus

**What to build:** Add the complete Volume → Sector → Volume interaction slice. A user can focus a Sector, descend to its exhaustive 8×8 Page grid, inspect every Page's independent semantic dimensions, and ascend with exact restoration at every tier and profile.

**Blocked by:** 04: Present the first Atlas Volume screen.

**Status:** superseded

**Superseded by:** [Volmap focused TUI implementation specification](../../volmap-tui-focused-inspector/implementation-spec.md).

- [ ] `Enter` and mouse activation commit the focused Sector into the typed Atlas trail; `Esc`, non-editing `Backspace`, and the breadcrumb return to Volume with the exact Sector focus and semantic anchor restored.
- [ ] Sector renders all 64 Pages once in ascending physical order and preserves allocation, exhaustive physical-type code, known/unknown/not-applicable occupancy, findings, focus, selection, and attribution independently.
- [ ] The Page focus rover is a true clamped 8×8 grid: horizontal movement never crosses row boundaries, vertical movement retains the column, and no edge wraps.
- [ ] Filtered, unreadable, unsupported, encrypted-opaque, diagnostic-bearing, and not-yet-enriched Pages remain visible, focusable, and valid navigation targets.
- [ ] Renderer-produced controls, focus edges, hit regions, legends, descriptors, and scroll extents remain equivalent across all tiers and presentation profiles.
- [ ] Paired keyboard/mouse traces reach identical trail, focus, selection, and effects; resize and stale commits cannot change Entity identity.
- [ ] The Sector subset of the 36 core goldens and `PAR-STATE`/`PAR-RENDER` assertions passes with Page/Sector file-class-table attribution preserved.
