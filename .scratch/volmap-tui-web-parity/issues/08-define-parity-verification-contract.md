Label: wayfinder:grilling
Type: grilling
Status: open
Assignee: unassigned
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Define the shared projection boundary for terminal parity](02-define-shared-projection-boundary.md), [Define the TUI navigation, focus, and history state model](03-define-navigation-focus-history-model.md), [Choose the terminal rendering architecture and dependency boundary](04-choose-rendering-architecture.md), [Define automatic enrichment and immutable-revision transitions](05-define-enrichment-revision-lifecycle.md), [Define semantic color, glyph, and fallback mappings](06-define-semantic-terminal-rendering.md), [Set volume viewport and rendering resource budgets](07-set-viewport-resource-budgets.md)

# Define the TUI parity verification contract

## Question

What evidence must prove the redesigned TUI satisfies terminal interaction parity before implementation is accepted? Define shared-projection parity tests, navigation and immutable-revision state tests, deterministic PTY or terminal-buffer captures for 120×36, 80×24, and 60×20, ANSI/Unicode and monochrome/ASCII cases, keyboard/mouse equivalence, resize and cancellation races, large-volume responsiveness, fragmented slot distributions, findings and coverage, non-disclosure, web-regression gates, and the boundary between stable semantic assertions and brittle pixel snapshots.
