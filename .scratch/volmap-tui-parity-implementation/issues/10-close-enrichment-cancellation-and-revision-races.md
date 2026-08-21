# 10: Close enrichment cancellation and revision races

**What to build:** Complete the Page-enrichment lifecycle under cancellation, navigation, publication, stale-base, invalidation, worker, and quit races. The old exact scene remains authoritative unless an active matching completion or explicit revision-offer action completes a whole-trail adoption.

**Blocked by:** 09: Automatically enrich Pages and adopt exact revisions.

**Status:** ready-for-agent

- [ ] Ready terminal input is reduced before a simultaneous worker signal; cancellation sets the token and revokes adoption authority before any trail transition.
- [ ] Atlas drains one cancelled worker, retains only the current replaceable next intent, admits no overlapping Page work, and never accumulates a FIFO queue.
- [ ] Page cancellation before publication creates no revision; allowed linked-target cancellation/resource results publish only a validated prefix under the shared target-specific contract.
- [ ] Publication-before-cancellation remains immutable but cannot auto-adopt after deactivation; wrong request, visit, Page, target, snapshot, base, or late progress cannot change the scene.
- [ ] Atlas retains at most one exact same-snapshot revision offer for late publication, failed adoption, or stale-base recovery; explicit adoption uses that exact key and current trail without a `latest` lookup.
- [ ] Explicit retry is available only for accepted recoverable outcomes, starts one new exact-base request, and never loops on a redraw or timer.
- [ ] Final source invalidation precedence, terminal snapshot overlay, head arbitration, one-successor publication, diagnostic idempotence, and retained old revisions hold for every candidate path.
- [ ] Quit and terminal faults deactivate, cancel, and join the one worker before teardown; no detached volume access or exit-time adoption survives.
- [ ] Named and seeded race traces prove one displayed revision, bounded request/progress/offer state, valid Atlas ancestry, and adoption only through an exact active match or explicit offer.
