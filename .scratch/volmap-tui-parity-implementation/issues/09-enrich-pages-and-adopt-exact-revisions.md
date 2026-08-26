# 09: Automatically enrich Pages and adopt exact revisions

**What to build:** Deliver the successful automatic Page-enrichment slice. Opening an eligible Page starts one bounded exact-base attempt, shows trusted progress over the unchanged old scene, publishes through the Projection workspace, and atomically adopts only the explicitly returned immutable revision. Follow [Define automatic enrichment and immutable-revision transitions](../../volmap-tui-web-parity/issues/05-define-enrichment-revision-lifecycle.md).

**Blocked by:** 07: Open the exhaustive Page workspace.

**Status:** superseded

**Superseded by:** [Volmap focused TUI implementation specification](../../volmap-tui-focused-inspector/implementation-spec.md).

- [ ] Page projection supplies the exhaustive shared eligibility disposition; Atlas starts work only for eligible targets and never reconstructs eligibility from allocation, messages, or browser predicates.
- [ ] Entering Page creates a visit identity and emits at most one automatic attempt per visit, exact base, and target; redraw, commit, resize, filter, scroll, region, overlay, and progress do not retrigger it.
- [ ] One bounded worker executes synchronous cooperative enrichment with the exact base handle and emits newest-valid trusted progress plus exactly one terminal result.
- [ ] The complete visible scene remains on the exact base revision during work; progress percentages appear only with a trusted total and matching monotonic counts.
- [ ] Valid, diagnostic-bearing, resource-limited publishable, and already-committed outcomes use the workspace's final source/head validation, immutable publication, one successor, and idempotence rules.
- [ ] A matching active completion reprojects and validates the whole Atlas trail, then atomically adopts the exact returned revision while preserving identities, focus, filter, finding, overlay, active region, and semantic anchors.
- [ ] Any checkout, projection, ancestry, identity, or scene-building failure rolls back to the wholly old scene without mixed caches or facts.
- [ ] Matching `Published` and `Unchanged` state/projection tests pass, and existing web enrichment behavior remains observably unchanged.
