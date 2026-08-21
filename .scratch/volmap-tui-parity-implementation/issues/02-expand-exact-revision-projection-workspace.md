# 02: Expand the exact-revision Projection workspace

**What to build:** Add the presentation-neutral, exact-revision Projection workspace beside the current adapters so Atlas and web can consume one typed source of truth. Follow D1 in the [implementation specification](../../volmap-tui-web-parity/implementation-spec.md) and [Define the shared projection boundary for terminal parity](../../volmap-tui-web-parity/issues/02-define-shared-projection-boundary.md). This is the expand step of the shared refactor; existing adapters remain operational.

**Blocked by:** 01: Freeze the parity corpus and evidence matrix.

**Status:** ready-for-agent

- [ ] The workspace exposes exact immutable checkout and closed typed projection operations; no operation silently substitutes a latest revision.
- [ ] Every projection frame carries one snapshot/revision plus validity, outcome, coverage, typed diagnostics with affected Entity references, and its typed result.
- [ ] Bounded Volume windows are contiguous and exhaustive when followed, and every projected Sector contains exactly 64 physical-order Pages.
- [ ] Shared Page facts preserve allocation, physical type, known/unknown occupancy, detail disposition, TDE state, and Page/Sector file-class-table attribution without adapter formatting.
- [ ] One atomic Page result contains its facts, complete safe Slot directory, and exhaustive 16,344-byte geometry derived only from a validated slotted Page.
- [ ] Immutable revision retention, head identity, snapshot invalidation state, deterministic ordering, and transport-neutral error types are available behind the workspace seam.
- [ ] Raw bytes, decoder structures, paths, HTTP types, cursors, terminal presentation, and adapter navigation state do not cross the seam.
- [ ] The applicable `PAR-PROJECTION` and disclosure gates pass while every existing adapter continues to pass its merge-base behavior unchanged.
