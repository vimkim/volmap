# 03: Observe a bounded resident-set scan

**What to build:** An authorized client requests a real resident-set scan and receives semantic BCB observations with a valid complete or explicitly partial result, while observation leaves page-buffer behavior unchanged.

**Blocked by:** 02 — Attach securely to the correct server.

**Status:** completed

- [x] Revalidate scalar access and lifetime assumptions against the implementation base. Use a private bounded sampling boundary and producer-owned scalar samples. Never fix/load a page, copy page contents, read disk contents, probe DWB/TDE, traverse replacement lists, change accounting or introduce hot-path instrumentation. No BCB mutex or page latch remains held during serialization or writes.
- [x] Emit volid, pageid, latch_mode (none/read/write/flush), waiter_present, fix_count, dirty, flushing, async_flush_requested, to_vacuum, page_lsa, oldest_unflush_lsa and semantic page_kind. Decode latch mode/waiters/fix count from one atomic word load, not separate reloading getters.
- [x] Decode lru_zone (lru1/lru2/lru3/void/invalid), lru_list_kind (shared/private/none/invalid) and nullable kind-local index from one flags sample. Shared indices precede private indices; subtract shared count for private membership, check the upper topology bound and map out-of-range to invalid/null. Non-LRU zones map to none/null. Never expose native list counters, packed indices, quotas or pointers.
- [x] Use the frozen schema for safe unknown values, page kinds and LSAs. Bind each scan to its incarnation and increasing scan_seq, emit start header and end/count/explicit-truncation footer, count emitted records before deduplication, and describe the interval without invented per-record timestamps or atomic-pool claims.
- [x] Enforce 65,536 visited slots and emitted records each, 64 MiB total framed bytes, 4 KiB record/control frames including newline and depth 16. Reserve footer space before admitting another record. Each slot is visited at most once; reaching a cap exactly at full traversal can remain complete, but stopping early is partial.
- [x] Rotate the next start beyond the visited span after truncation and prove progress on a stable oversized pool. Never merge captures or treat partial omissions as nonresident; repeated VPIDs remain ambiguous. Valid footer completion is required before any new capture becomes usable.
- [x] Enforce the 100 ms server scan-start floor and 100 ms traversal/serialization elapsed deadline checked between slots, including elapsed backpressure. Keep output buffering at 64 KiB, disconnect after 250 ms without write progress and bound the entire scan exchange including drain/footer to 2 s. Stream incrementally rather than allocating a whole scan. These are scheduling-aware bounds, not real-time guarantees.
- [x] Cancel unnecessary work on disconnect, stall or shutdown and free producer resources. Preserve two-client admission, rate-limited/busy refusals and all attachment security from ticket 02. Safety limits ship here; ticket 04 adds adversarial evidence rather than enabling safeguards later.
- [x] Test observable encoded results at the socket and deterministic scan/serializer boundary: empty/complete/partial, all semantic tuples, invalid/topology boundaries, exact counts, footer reservation, limit boundaries, rotation, deadlines and cancellation. Record nonzero test execution and a real-server scan in debug and release. Controlled dirty/eviction oracles follow in ticket 05.

## Source and scope

Implements the approved CUBRID producer specification for CBRD-27398 in this feature tracker. Read its full contract and linked decision authority before implementation. The public protocol is the main test boundary; focused deterministic scan/serializer checks supplement real I/O. This ticket does not add resident-page capture, AOUT or event history, native LRU telemetry, or Volmap UI work.

## Comments

2026-09-09: Published after the user approved the eight-ticket breakdown and blocking edges. Ready-for-agent describes triage readiness; blockers and acceptance criteria remain unsatisfied until evidenced.

2026-09-09 implementation clarification: after the proposed tradeoff was explained, the user invoked `$implement` to proceed. Use bounded BCB trylock; unobserved slots make coverage partial. Omit LSA/page_kind when safe scalar access cannot be established; missing fields mean unknown under the frozen v1 schema, not null. Idle non-flushing slots can supply these fields without fixing a page.

2026-09-09 completion: committed as `3ae2404dc7832243ca9be7dc160768222d0594b4`. Debug and release each passed all 27 CTest entries, 51 inspector cases and 3 separately enabled credential cases. Three real-server captures per mode were complete with 4096 visited slots and 58 records. Standards and specification reviews have no remaining findings. Evidence: [verification manifest](../verification/03/manifest.json) and [checkpoint](../verification/03/checkpoint.md).
