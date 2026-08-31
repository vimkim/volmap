# 03: Inspect Page record distribution

**What to build:** Extract the validated slotted-Page distribution into shared projection and add Page mode with bounded automatic Page enrichment. Follow F3 and the exact Page geometry contract in the [focused TUI implementation specification](../implementation-spec.md). The screen combines a proportional byte map with exhaustive, selectable structural rows.

**Blocked by:** [02: Add detailed Sector drill-down](02-add-detailed-sector-drill-down.md).

**Status:** implemented

- [x] A presentation-neutral shared projection derives geometry only from a validated slotted Page and covers all 16,344 content bytes exactly: header, sorted live records, every free interval, and the complete four-byte Slot directory.
- [x] Every Slot entry retains Slot id, entry range, record type, and allocated, empty, or deleted state independently of whether its record extent is live.
- [x] Web consumes the shared projection without changing existing `/page` JSON shape, browser behavior, distribution ordering, or serialization tests.
- [x] Entering a supported Page automatically requests Page enrichment when its distribution is absent, keeps the old exact revision visible, and publishes at most one active bounded worker request.
- [x] Adoption requires matching request identity, snapshot, exact base revision, and Page, then re-resolves the complete structural path before replacing the current view; cancellation and late results cannot adopt.
- [x] The Page byte bar and scrollable rows keep every exact region reachable while formatting only the visible window plus one screen before and after.
- [x] Live records are selectable by stable OID/Slot identity; header, free space, directory, tombstone, and empty rows remain visible but cannot open record interpretation.
- [x] Geometry conservation, unchanged-web, Page loading/failure/cancellation/adoption, distribution reachability, and Page goldens pass at all required sizes/profiles.
