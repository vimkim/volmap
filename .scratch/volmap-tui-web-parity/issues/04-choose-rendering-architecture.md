Label: wayfinder:grilling
Type: grilling
Status: open
Assignee: unassigned
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Prototype terminal interaction parity across Volume, Sector, and Page](01-prototype-terminal-interaction-parity.md), [Define the shared projection boundary for terminal parity](02-define-shared-projection-boundary.md)

# Choose the terminal rendering architecture and dependency boundary

## Question

Should production retain and deepen the current manual `crossterm` renderer, introduce a terminal widget/layout library such as Ratatui, or place a small repository-owned widget layer over `crossterm`? Decide using the accepted prototype, adaptive-layout complexity, focus and hit-testing needs, display-column correctness, static-musl and reproducible-release constraints, dependency/license/SBOM cost, testability, and the requirement to keep a deep terminal interface rather than scatter view logic across key handling and drawing code.
