Label: wayfinder:grilling
Type: grilling
Status: open
Assignee: unassigned
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Prototype terminal interaction parity across Volume, Sector, and Page](01-prototype-terminal-interaction-parity.md), [Choose the terminal rendering architecture and dependency boundary](04-choose-rendering-architecture.md)

# Set volume viewport and rendering resource budgets

## Question

What bounded viewport, caching, and redraw policy lets the TUI navigate complete large-volume mosaics while keeping input latency and memory predictable? Decide sector-card packing and scrolling, visible-window and overscan rules, resize invalidation, page-distribution row virtualization, stable focus across window changes, redraw triggers during enrichment, and measurable resource/latency budgets. Distinguish terminal rendering budgets from existing inspection operational budgets and never silently sample or omit sectors.
