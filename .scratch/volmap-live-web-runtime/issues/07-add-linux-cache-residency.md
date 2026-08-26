# 07: Add Linux page-cache residency

**What to build:** Implement W6's bounded, non-loading kernel-cache adapter and OS-cache overlay. Version one reports residency only.

**Blocked by:** 06: Add the loopback runtime broker and HTTP boundary.

**Status:** ready-for-agent

- [ ] Validated volume handles and Rust-projected physical file ranges are the only probe inputs; browser paths and unchecked arithmetic never reach the adapter.
- [ ] Linux uses `cachestat(2)` when available and `mincore(2)` over untouched read-only mappings as fallback; adjacent page requests are coalesced within a hard byte cap.
- [ ] Per-page result is exactly fully resident, partially resident, not resident, or unknown, with method, capture time, capability, and limitations.
- [ ] Unsupported OS/kernel, permission failure, mapping/range failure, truncated file, and replaced file yield unknown/capability state rather than false absence.
- [ ] No result claims OS dirty, writeback, eviction cause, access frequency, durability, or history.
- [ ] The main crate retains `unsafe_code = "forbid"`; any necessary FFI is isolated in one private safe-interface workspace crate with a reviewed safety proof and focused boundary tests.
- [ ] Probes do not touch mapped bytes, load absent pages, scan the entire volume, or retain unbounded mappings/history.
- [ ] Browser and adapter tests cover all semantic states, selected-first ordering, partial coverage, method fallback, stale state, and unsupported-platform compilation without relying on ambient host cache contents.
