Type: research
Status: resolved
Blocked by:

# Verify the page-buffer exposure surface on the candidate CUBRID branches

## Question

The charting survey ([CUBRID page-buffer exposure surface](../research/cubrid-pgbuf-exposure-surface.md)) ran on an OOS feature worktree (`CBRD-26067-storage-force-outline` @ `82a0a4bb1`) whose BCB packs latch mode, waiter flag, and fix count into one 64-bit atomic (`PGBUF_ATOMIC_LATCH`). Verify which of the surveyed facts hold on the two real candidate branches: plain `develop` (worktree `/home/vimkim/gh/cb/develop`) and the volmap-pinned `feat/oos` commit `e1e651debf6cc100172bde96603b17424f9c135a`.

Per branch, with file:line evidence:

1. BCB field layout — atomic latch vs separate `latch_mode`/`fcnt`/waiter fields; the `flags` word and its dirty/flushing/victim/vacuum bits; zone/LRU packing; `oldest_unflush_lsa`; whether a coherent (latch mode, fix count) read requires the BCB mutex.
2. Lock-free whole-pool scan precedents — `pgbuf_scan_bcb_table`, `pgbuf_peek_stats`, `pgbuf_search_hash_chain_no_bcb_lock` — presence and shape.
3. The `pgbuf_monitor_locks` parameter and its NDEBUG gating idiom.
4. `controller.hpp` / `ENABLE_CONTROLLER` AF_UNIX subsystem presence.
5. The `memmon` NET_SERVER + utility pattern presence.
6. The DWB `slots_hashmap` per-VPID probe.
7. `pgbuf_dump` rot state under `CUBRID_DEBUG`.

Conclude with the per-branch differences that would change the inspector design, and whether one wire contract can serve both branches unchanged.

## Comments

## Answer

Verified 2026-08-21 against develop @ `1befe4b40` (worktree `/home/vimkim/gh/cb/develop`)
and pinned feat/oos `e1e651d` (via `git show`, no checkout). Full evidence:
[Branch exposure parity](../research/branch-exposure-parity.md).

**Verdict: all seven surveyed facts hold on BOTH branches; the surface is
near-identical.** `page_buffer.c` differs by 29 lines in 3 hunks;
`controller.hpp` and both DWB files are byte-identical.

- The baseline caveat is wrong: **develop already has `PGBUF_ATOMIC_LATCH`**
  (landed with CBRD-26425 `58cef8e01`, ancestor of both). No separate
  `latch_mode`/`fcnt` fields on either branch; one atomic load of
  `atomic_latch.raw` gives a coherent (latch, waiter, fcnt) triple with no
  BCB mutex — on both branches.
- `pgbuf_search_hash_chain_no_bcb_lock` exists on both (develop
  `page_buffer.c:7737`, oos `:7735`), used by the shipping lock-free RO fix path.
- Scan precedents, `pgbuf_monitor_locks` NDEBUG idiom, `ENABLE_CONTROLLER`
  AF_UNIX subsystem, memmon pattern, DWB probe, and the identically rotted
  `CUBRID_DEBUG` `pgbuf_dump`: parity, small line drift only.
- The one producer-material divergence: OOS inserts `PAGE_OOS` mid-enum
  (`storage_common.h:159`), shifting raw ptype values ≥ 8 by +1. The wire
  contract must carry a semantic page-kind vocabulary (never raw ptype), with
  a per-branch enum→kind table and an `oos` kind develop never emits.
- **One wire contract serves both branches unchanged**; a develop-landed
  producer should cherry-pick onto feat/oos with only `PRM_LAST_ID`-tail and
  `PAGE_OOS`-switch conflicts.
