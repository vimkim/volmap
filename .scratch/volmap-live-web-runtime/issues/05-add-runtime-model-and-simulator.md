# 05: Add the runtime model and deterministic simulator

**What to build:** Add W4's pure runtime capability, polling, request-adoption, coverage, resident-correspondence, and overlay state using only a deterministic in-memory source. Do not attach to CUBRID or the kernel in this ticket.

**Blocked by:** 02: Establish the React compatibility viewer.

**Status:** ready-for-agent

- [ ] The one reducer owns `disabled`, `connecting`, `active`, `stale`, `unavailable`, `refused`, and `incompatible` capabilities independently for each source.
- [ ] Freshness derives from capture age and cadence: fresh through two intervals and stale afterward, with age always visible.
- [ ] Immutable observation scope keys bind source, request/pause epochs, database identity, incarnation, selected VPID, requested scope, and overlay.
- [ ] Route, viewport scope, overlay, pause epoch, identity, incarnation, and restart transitions reject incompatible in-flight responses even when abort loses the race.
- [ ] Pause freezes disk/runtime adoption together and retains at most the latest offer per source; resume adopts coherent latest offers and clears invalid correspondence.
- [ ] Hidden state schedules no work; visible restoration requests fresh state; failed sources back off without erasing ordinary inspection.
- [ ] The simulator produces active, stale, refused, incompatible, restart, divergent, partial, delayed, and out-of-order cases under controlled time.
- [ ] Allocation/occupancy/finding stays visible under exactly one additional overlay; border+badge, tint+pattern, and split-cell variants render identical semantics without color-only meaning.
- [ ] Reducer/selector traces and browser scenarios cover every valid and rejected transition; polling ticks are not screen-reader announcements.
