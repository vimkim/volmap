# Branch exposure parity: develop vs pinned feat/oos

Research for [ticket 01](../issues/01-verify-branch-exposure-surface.md), 2026-08-21.
Re-verifies the [CUBRID page-buffer exposure surface](cubrid-pgbuf-exposure-surface.md)
survey (taken on OOS worktree `82a0a4bb1`) against the two real candidate branches:

- **develop** — worktree `/home/vimkim/gh/cb/develop` @ `1befe4b40`
  ("[CBRD-27073] Fix log applier error message buffer overflow (#7612)").
  Cited as `develop <file>:<line>`.
- **feat/oos pinned** — commit `e1e651debf6cc100172bde96603b17424f9c135a`
  ("hotfix(serial): pass OOS recdes consumption policy", 2026-08-14), inspected
  read-only via `git -C /home/vimkim/gh/cb/oos-storage show e1e651d:<path>`
  (the `oos-storage` worktree contains it as an ancestor; nothing was checked
  out). Cited as `oos <file>:<line>` (line numbers inside the shown blob).

## Verdict

**The two branches are near-identical across the entire surveyed exposure
surface.** `src/storage/page_buffer.c` differs by 29 lines in 3 hunks (one
cosmetic, one develop-only helper, one OOS-only `case PAGE_OOS:`);
`controller.hpp` and both DWB files are byte-identical; the system-parameter
machinery differs only by three newer develop-side parameters unrelated to
pgbuf. The baseline survey's caveat — "develop may still have separate
`bcb->latch_mode` / `bcb->fcnt` fields" — is **wrong**: the atomic latch landed
on develop via CBRD-26425 (#6704, `develop` commit `58cef8e01`), which is an
ancestor of both branches. All seven surveyed facts hold on both branches.
The single materially design-relevant divergence is the `PAGE_TYPE` enum:
OOS inserts `PAGE_OOS` mid-enum, shifting raw ptype values ≥ 8 by +1.

## The seven points, per branch

### 1. BCB field layout — PARITY (byte-identical struct)

`struct pgbuf_bcb` is textually identical: develop `page_buffer.c:513-545`,
oos `page_buffer.c:511-543`. Both have:

- `PGBUF_ATOMIC_LATCH atomic_latch` (develop :520, oos :518);
  `typedef std::atomic<uint64_t> PGBUF_ATOMIC_LATCH` (develop :367, oos :365);
  `union pgbuf_atomic_latch_impl {uint64_t raw; struct {latch_mode;
  waiter_exists; fcnt}}` (develop :501-508, oos :499-506).
  **Neither branch has separate `latch_mode`/`fcnt` BCB fields.**
- Accessors `get_fcnt` / `get_waiter_exists` / `get_latch` / `get_impl`
  (develop :1467/1475/1483/1491, oos :1465/1473/1481/1489). One relaxed-load
  of `raw` yields a coherent (latch mode, waiter flag, fix count) triple —
  **a coherent (latch mode, fix count) read needs no BCB mutex on either
  branch**; the mutex remains needed only for waiter-queue and
  transition-consistent views, which the inspector does not take.
- `volatile int flags` (develop :521, oos :519) carrying dirty
  `0x80000000` / flushing `0x40000000` / victim-direct `0x20000000` /
  invalidate-victim `0x10000000` / move-to-LRU-bottom `0x08000000` /
  to-vacuum `0x04000000` / async-flush `0x02000000` (develop :224-241,
  oos :224-241 — same lines, region identical), plus zone bits
  (develop :197-211, `PGBUF_LRU_1/2/3_ZONE`, `PGBUF_INVALID_ZONE`,
  `PGBUF_VOID_ZONE`) and the low-16-bit LRU index
  (`PGBUF_LRU_INDEX_MASK` develop :182). Extractors `PGBUF_GET_ZONE` /
  `PGBUF_GET_LRU_INDEX` at :215-216 on both.
- `LOG_LSA oldest_unflush_lsa` (develop :543, oos :541) and
  `PGBUF_IOPAGE_BUFFER *iopage_buffer` (develop :544, oos :542).
- Latch-mode enum `PGBUF_NO_LATCH=0 … PGBUF_LATCH_INVALID=4` as
  `enum:uint16_t`: `page_buffer.h:190-197` on both branches.

### 2. Lock-free whole-pool scan precedents — PARITY

| Primitive | develop | oos |
|---|---|---|
| `pgbuf_scan_bcb_table ()` ("scan bcb table … with no bcb mutex") | `page_buffer.c:17279` | `:17257` |
| `pgbuf_start_scan` (takes only `show_status_mutex`, develop :17386) | `:17367` | `:17346` |
| `pgbuf_peek_stats` (second lock-free full scan) | `:14686` | `:14684` |
| `pgbuf_search_hash_chain` (returns with BCB mutex held) | `:7547` | `:7545` |
| **`pgbuf_search_hash_chain_no_bcb_lock`** (plain chain walk, no locks) | `:7737` | `:7735` |
| … used by the lock-free RO fix path | `:7678` | `:7676` |
| `PGBUF_FIND_BCB_PTR(i)` whole-table iteration | `:135` | `:135` |
| AOUT hashmap (`mht_create "PGBUF_AOUT_HASH"`) | `:5825` | `:5823` |

`pgbuf_search_hash_chain_no_bcb_lock` **exists on both branches** — it (and
the atomic latch) came from CBRD-26425 `58cef8e01`, shared ancestry. The
only content difference in this area: OOS buckets `case PAGE_OOS:` into
`num_data_pages` inside `pgbuf_scan_bcb_table` (oos :17314); develop's switch
(develop :17334-17336) has no such case. `perf_monitor.c` is identical where
it matters: `pgbuf_peek_stats` call at :1976, opt-in
`xperfmon_server_copy_stats (…, bool need_pgbuf_stat)` at :1020 on both.

### 3. `pgbuf_monitor_locks` parameter and NDEBUG idiom — PARITY

- Name `"pgbuf_monitor_locks"`: `system_parameter.c:676` on both.
- `PRM_ID_PB_MONITOR_LOCKS`: `system_parameter.h:404` on both.
- `prm_Def[]` entry `(PRM_FOR_SERVER | PRM_HIDDEN)`, default false:
  develop `system_parameter.c:4214`, oos `:4187`.
- Cached file-static bool + macro wrappers: develop `page_buffer.c:948`
  (macros :952-960), oos `:946` (macros :950-958).
- The exact NDEBUG idiom (forced true in debug, param-read in release) is
  textually identical: develop `page_buffer.c:1676-1682`, oos `:1674-1680`.

One branch-relevant nit for "add a new parameter": the enum tail differs.
develop `PRM_LAST_ID = PRM_ID_PLAN_CACHE_BIND_SENSITIVITY`
(`system_parameter.h:554`); oos `PRM_LAST_ID = PRM_ID_ENABLE_LAZY_PREDICATE_READ`
(`:548`). develop carries three post-branch parameters
(`statistics_sampling_threshold_pages`, `statistics_sample_pages`,
`plan_cache_bind_sensitivity`). A new `PRM_ID_*` goes at each branch's own
tail; a develop-targeted patch will not apply cleanly to feat/oos at the enum
tail, but the mechanical rule (`system_parameter.h:92-95`) is the same.
(The baseline's `PRM_LAST_ID = PRM_ID_BESTSPACE_SHARD_COUNT` was specific to
the surveyed worktree; it matches neither candidate branch.)

### 4. `controller.hpp` / `ENABLE_CONTROLLER` — PARITY (byte-identical)

`src/connection/controller.hpp` is **byte-identical** between develop and the
pinned commit (AF_UNIX `SOCK_DGRAM` open at :139, bind :146). The coordinator
usage is line-identical too: `#if defined (ENABLE_CONTROLLER)` at
`coordinator.cpp:71, 98, 1131, 1355` and `coordinator.hpp:35, 121, 247, 376`
on both; socket path `"/tmp/cub_server_<pid>_coordinator.sock"` at
`coordinator.cpp:73` on both. `ENABLE_CONTROLLER` is **not** a CMake option on
either branch (develop options `CMakeLists.txt:71-77`, oos `:68-75` — the only
difference is the OOS-only `option(UNIT_TEST_OOS … ON)` at oos :74). Compiled
out by default on both.

### 5. `memmon` NET_SERVER + utility pattern — PARITY

| Layer | develop | oos |
|---|---|---|
| `NET_SERVER_MMON_GET_SERVER_INFO` X-macro | `network.h:273` | `:273` |
| Dispatch `= smmon_get_server_info` | `network_sr.c:759` | `:762` |
| Handler + `mmon_is_memory_monitor_enabled ()` gate | `network_interface_sr.cpp:12125, :12135` | `:12149, :12159` |
| Client stub `mmon_get_server_info` | `network_interface_cl.c:11642` | `:11649` |
| `memmon (UTIL_FUNCTION_ARG *)` utility | `util_cs.c:5078` | `:5066` |
| `#if !defined(WINDOWS)` whole-file gate | `memory_monitor_sr.cpp:23` | `:23` |

Same six edit sites, same shape, small line drift only.

### 6. DWB `slots_hashmap` per-VPID probe — PARITY (byte-identical files)

`double_write_buffer.cpp` and `.hpp` are **byte-identical** between develop
and the pinned commit. Shared line cites: `dwb_hashmap_type`
(lockfree_hashmap keyed by VPID) `:258`, member `slots_hashmap` `:279`,
`dwb_read_page (thread_p, vpid, io_page, success)` `:3979`, hashmap init
`:1222`, `DWB_SLOT` with `VPID vpid` `double_write_buffer.hpp:36`. The
baseline's caveats (full-page copy, entry mutex held by `find()`, needs a
`THREAD_ENTRY`, N-probe cost profile) carry over to both branches unchanged.

### 7. `pgbuf_dump` rot under `CUBRID_DEBUG` — PARITY (identically rotted)

The `#if defined(CUBRID_DEBUG)` block is develop `page_buffer.c:11219-11488`,
oos `:11217-11486`; `pgbuf_dump` develop `:11303`, oos `:11301`;
`pgbuf_dump_if_any_fixed` develop `:11258`, oos `:11256`. The block was
*partially* modernized on both (it already reads
`get_latch (&bufptr->atomic_latch)` / `get_fcnt` — develop :11361, :11368,
:11373), but three compile-breakers remain, identical on both branches:

1. `bufptr->fcnt` — field no longer exists (develop :11349, oos :11347).
2. `zone_str = pgbuf_latch_mode_str (bufptr->zone)` — dead field **and**
   wrong stringifier (develop :11369, oos :11367).
3. `consistenet_str = …` assigned but only `consistent_str` is declared
   (develop :11370 vs declaration :11310; oos :11368). The same
   typo'd assignment sits in `pgbuf_unfix_all`'s CUBRID_DEBUG branch
   (develop :3284, oos :3282).

Live helpers `pgbuf_latch_mode_str` / `pgbuf_zone_str` /
`pgbuf_consistent_str` exist on both (develop declarations :1260-1262), and
`pgbuf_is_consistent` (develop :1214) still re-reads the page from disk.

## Complete divergence list (pgbuf exposure surface)

Everything not listed here is identical (modulo ≤ ~30-line drift).

1. **`PAGE_TYPE` enum values diverge from 8 up** — the only producer-material
   difference. OOS inserts `PAGE_OOS` between `PAGE_OVERFLOW` and `PAGE_AREA`
   (oos `storage_common.h:159`); develop has no `PAGE_OOS`
   (develop `storage_common.h:151-166`). Raw values: develop
   `PAGE_AREA=8 … PAGE_VACUUM_DATA=13`; oos `PAGE_OOS=8, PAGE_AREA=9 …
   PAGE_VACUUM_DATA=14`.
2. `pgbuf_scan_bcb_table` on OOS buckets `case PAGE_OOS:` as a data page
   (oos `page_buffer.c:17314`); develop switch lacks it (develop :17334-17336).
3. develop-only helper `pgbuf_mark_page_for_lru_bottom` (develop
   `page_buffer.c:16152`, declared `page_buffer.h:503`) — marks a fixed page
   so its unfix drops the BCB to the LRU bottom; built for statistics-style
   long scans that must not pollute the working set. Absent at the pinned
   commit. Irrelevant to a non-fixing inspector, but the right tool if any
   part of the design ever fixes pages.
4. Cosmetic: `PGBUF_SHOULD_IGNORE_UNFIX` macro parenthesization
   (develop `page_buffer.c:283-296` region vs oos same region).
5. `PRM_LAST_ID` tail differs (point 3 above) — merge-conflict surface for a
   new parameter, not a design difference.

## Design consequences

- **One wire contract serves both branches.** Every capture-side primitive
  the design leans on — the atomic latch as one coherent 64-bit read, the
  `flags` word and its extractors, the lock-free whole-pool scan, the
  no-lock hash-chain probe, `oldest_unflush_lsa`, the `pgbuf_monitor_locks`
  gating idiom, the controller and memmon transport precedents, the DWB
  probe — is textually identical on develop and the pinned feat/oos commit.
  The producer code is the same code on both branches; a develop-landed
  inspector patch should cherry-pick onto feat/oos with at most enum-tail
  (`PRM_LAST_ID`) and `PAGE_OOS`-switch conflicts.
- **The wire contract must not ship raw `ptype` bytes.** With `PAGE_OOS`
  inserted mid-enum, every raw ptype ≥ 8 means a different page kind per
  branch. The contract's page-kind field must be a wire-owned semantic
  vocabulary (which the design reference already intends), and the producer
  maps `PAGE_TYPE` → wire kind through a per-branch table. The vocabulary
  needs an `oos` page kind that develop simply never emits — consumers must
  treat unknown/absent kinds as valid. This is a one-table difference in the
  producer, not a contract fork.
- **The (latch mode, fix count) coherence question is closed.** On both
  branches one atomic load of `atomic_latch.raw` yields latch mode + waiter
  flag + fix count with no torn combinations and no BCB mutex. The baseline's
  fallback concern (develop needing the mutex for a coherent pair) is moot.
- **`pgbuf_search_hash_chain_no_bcb_lock` is available on both branches** for
  a per-VPID point probe, already trusted by the shipping lock-free RO fix
  path on both.
- The remaining divergences (develop's `pgbuf_mark_page_for_lru_bottom`,
  three newer develop parameters, macro cosmetics) do not touch the inspector
  design. The OOS snapshot bucketing hunk only matters if the design reuses
  `PGBUF_STATUS_SNAPSHOT` aggregates, which it does not plan to.
