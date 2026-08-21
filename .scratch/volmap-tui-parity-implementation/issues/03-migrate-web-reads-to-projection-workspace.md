# 03: Migrate web reads to the Projection workspace

**What to build:** Move the web adapter's read-only Volume, Sector, Page, diagnostics, coverage, Slot, and distribution resource construction onto the accepted Projection workspace while preserving its observable API and browser behavior. This is the migrate/contract step for web-private semantic derivation described by D1 in the [implementation specification](../../volmap-tui-web-parity/implementation-spec.md); it does not redesign web interaction.

**Blocked by:** 02: Expand the exact-revision Projection workspace.

**Status:** ready-for-agent

- [ ] Handler tests prove the exact existing Volume, Sector, and Page JSON shapes before and after enrichment, including top-level Slots/distribution, attribution, record interpretation, diagnostics, coverage, and path-free disclosure.
- [ ] Exact revision envelopes, bounded pagination and revision-bound cursors, retained old-revision reads, and terminal invalidation overlays remain unchanged.
- [ ] Existing `202` receipts, `Location` and result URLs, stale/invalidation conflicts, admission/resource refusal, unsupported responses, and diagnostic-bearing successful revisions remain unchanged.
- [ ] The web adapter consumes shared typed occupancy, attribution, diagnostics, Page geometry, and Slot states without recomputing or parsing them from labels or messages.
- [ ] The superseded web-private Page distribution derivation and duplicated format constants are removed after all web callers use the shared implementation.
- [ ] The merge-base Rust web, CLI, JSON/JSONL, deterministic HTML, schema, ordering, and disclosure tests pass without rebaselining.
