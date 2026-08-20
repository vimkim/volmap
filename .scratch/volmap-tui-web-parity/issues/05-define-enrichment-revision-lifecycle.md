Label: wayfinder:grilling
Type: grilling
Status: open
Assignee: unassigned
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Define the shared projection boundary for terminal parity](02-define-shared-projection-boundary.md), [Define the TUI navigation, focus, and history state model](03-define-navigation-focus-history-model.md)

# Define automatic enrichment and immutable-revision transitions

## Question

What exact lifecycle begins when a user opens a page whose supported detail has not yet been requested? Specify bounded job admission, visible loading state, cancellation and late completion, success and diagnostic outcomes, explicit adoption of the returned immutable revision, selection/focus restoration, navigation back to older context, input invalidation, and behavior when detail is unsupported, opaque, already complete, or resource-limited. Preserve the web contract's no-silent-mixing rule without importing browser routing into the TUI.
