# 02: Add detailed Sector drill-down

**What to build:** Add structural Volume → Sector descent and a detailed Sector mode containing all 64 Pages. Follow F2 and the interaction model in the [focused TUI implementation specification](../implementation-spec.md). Sector mode must make exact occupancy easy to compare while retaining enough focused context to choose the correct Page.

**Blocked by:** [01: Build the Volume occupancy mosaic](01-build-volume-occupancy-mosaic.md).

**Status:** ready-for-agent

- [ ] `Enter` on a focused Sector opens its Sector mode, while `Esc` and `Backspace` restore the exact Volume focus and scroll anchor.
- [ ] The Page rover is a row/column-clamped 8×8 grid: horizontal movement never crosses a row boundary and vertical movement never changes column at an edge.
- [ ] Every Page appears exactly once and shows its exact occupied percentage, `?`, or `-`; at 80 columns and wider the cell also shows compact physical type.
- [ ] At 60–79 columns the focused descriptor exposes type, allocation, exact occupied/free percentages, finding, and file/class/table attribution without removing any Page from the grid.
- [ ] `[`/`]`, mouse activation, and wheel input call the same semantic actions as their keyboard equivalents and preserve structural ascent behavior.
- [ ] Volume occupancy buckets and Sector percentages are derived from the same projection and agree for known zero, the existing 7/93 case, all positive buckets through 100, unknown, and not-applicable.
- [ ] Sector session traces and semantic goldens cover descent, ascent restoration, edge clamping, resize, sibling movement, and all three required presentation sizes/profiles.
