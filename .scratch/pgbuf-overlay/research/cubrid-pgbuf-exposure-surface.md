# CUBRID page-buffer exposure surface

Charting survey, 2026-08-21. Surveyed tree:
`/home/vimkim/gh/cb/CBRD-26067-storage-force-outline` @ `82a0a4bb1`
(merge of `origin/feat/oos` into the branch).

Two caveats:

1. **This is not plain develop.** It carries the OOS work and a reworked BCB
   latch: `latch_mode` + `fcnt` are packed into a single 64-bit
   `std::atomic` (`PGBUF_ATOMIC_LATCH`), plus a lock-free RO fix path.
   Develop may still have separate `bcb->latch_mode` / `bcb->fcnt` fields.
   Re-verification per branch is map ticket 01.
   **Superseded 2026-08-21 by ticket 01: the concern was unfounded — develop
   already has `PGBUF_ATOMIC_LATCH` (CBRD-26425), and all surveyed facts hold
   on both branches. See [Branch exposure parity](branch-exposure-parity.md).**
2. The tree has stale `*.c~`/`*.h~` backups; only live files are cited.

## 1. Per-page state a BCB holds

`struct pgbuf_bcb` — `src/storage/page_buffer.c:511-542`:

| Field | Line | Notes |
|---|---|---|
| `pthread_mutex_t mutex` / `int owner_mutex` | 513-515 | SERVER_MODE only |
| `VPID vpid` | 516 | the heatmap key |
| `PGBUF_ATOMIC_LATCH atomic_latch` | 517 | latch mode + fix count + waiter flag in one 64-bit word |
| `volatile int flags` | 518 | dirty/flush/victim flags + zone + LRU index, packed |
| `THREAD_ENTRY *next_wait_thrd` | 520 | waiter queue head |
| `THREAD_ENTRY *latch_last_thread` | 523 | last latch acquirer |
| `tick_lru_list`, `tick_lru3` | 528-531 | LRU age/position |
| `volatile int count_fix_and_avoid_dealloc` | 532 | packed dual counter |
| `int hit_age` | 538 | quota math |
| `LOG_LSA oldest_unflush_lsa` | 540 | oldest-unflush LSA |
| `PGBUF_IOPAGE_BUFFER *iopage_buffer` | 541 | page image; page LSA = `iopage.prv.lsa`, type = `iopage.prv.ptype` |

Latch modes — `src/storage/page_buffer.h:190-197`: `PGBUF_NO_LATCH=0,
PGBUF_LATCH_READ=1, PGBUF_LATCH_WRITE=2, PGBUF_LATCH_FLUSH=3,
PGBUF_LATCH_INVALID=4` (`enum:uint16_t`, packed into the atomic latch).

Atomic latch layout — `union pgbuf_atomic_latch_impl`
(`page_buffer.c:499-508`): `{uint64_t raw}` vs `{latch_mode; waiter_exists;
fcnt}`. Accessors `get_fcnt` (1465), `get_waiter_exists` (1473), `get_latch`
(1481). One atomic load yields a coherent (latch, waiter, fcnt) triple —
no torn combination for a snapshot reader.

BCB flags — `page_buffer.c:221-250`: `DIRTY 0x80000000`,
`FLUSHING_TO_DISK 0x40000000`, `VICTIM_DIRECT 0x20000000`,
`INVALIDATE_DIRECT_VICTIM 0x10000000`, `MOVE_TO_LRU_BOTTOM 0x08000000`,
`TO_VACUUM 0x04000000`, `ASYNC_FLUSH_REQ 0x02000000`.
Zones (same word) — `page_buffer.c:184-212`: LRU 1/2/3, `INVALID`, `VOID`;
low 16 bits are the LRU list index; extractors `PGBUF_GET_ZONE` (215),
`PGBUF_GET_LRU_INDEX` (216).

Ready-made read accessors: `pgbuf_bcb_get_zone` 15931, `get_lru_index`
15943, `is_dirty` 15956, `is_flushing` 16069, `is_async_flush_request`
16105, `is_to_vacuum` ~16139, `avoid_victim` 16159,
`should_avoid_deallocation` 16240, `get_pool_index` 16171.
From a `PAGE_PTR`: `pgbuf_get_lsa` 4913, `pgbuf_get_vpid` 5159,
`pgbuf_get_latch_mode` 5213, `pgbuf_get_page_ptype` 5255,
`pgbuf_get_fix_count` 14979, `pgbuf_is_page_fixed_by_thread` 13856.

Finding a BCB by VPID: hash table `pgbuf_Pool.buf_hash_table[]`
(`PGBUF_HASH_VALUE`, fn at 1523).
- `pgbuf_search_hash_chain` (7545) — returns with `bufptr->mutex` held; not
  for a passive inspector.
- **`pgbuf_search_hash_chain_no_bcb_lock` (7735) — plain chain walk, no
  locks, returns BCB or NULL; already used by the lock-free RO fix path
  (7669). The primitive a read-only inspector wants.**
- Whole-table iteration: `PGBUF_FIND_BCB_PTR(i)` (135-136) over
  `pgbuf_Pool.BCB_table` (762), `i < pgbuf_Pool.num_buffers`.
- AOUT (evicted-page history) hashmap exists (`mht_create` 5823) — a
  possible "recently evicted" layer later.

## 2. SHOW PAGE BUFFER STATUS today

Wiring: grammar `csql_grammar.y:7420-7422` → `show_meta.c:762`
(`only_for_dba = true`), registered `show_meta.c:1012` → `show_scan.c:245-249`
→ `pgbuf_start_scan` (`page_buffer.c:17346`).

Strictly aggregate — one row, 19 columns. Two data sources:

1. Per-thread sharded counters `PGBUF_STATUS` (`alignas(64)`, 393-402),
   array sized threads+1 (1846-1855), summed in `pgbuf_start_scan`
   (17369-17378); deltas vs `show_status_old` make the rate columns.
2. **`pgbuf_scan_bcb_table()` (17257)** — header comment: "scan bcb table to
   count snapshot data with no bcb mutex". Walks all `num_buffers` BCBs
   (17268), reading `ptype`, `vpid`, `flags` into locals (17271-17273),
   bucketing into `PGBUF_STATUS_SNAPSHOT` (406-416). No BCB/LRU/hash mutex —
   a deliberately racy lock-free full scan, shipping in release.

Synchronization: only `show_status_mutex` (830, taken 17364) serializing
concurrent SHOW callers, not protecting BCBs.

Second precedent: `pgbuf_peek_stats` (14684) — another lock-free whole-table
scan feeding perfmon gauges (`perf_monitor.c:1976`); note
`xperfmon_server_copy_stats(..., bool need_pgbuf_stat)` (`perf_monitor.c:1020`)
makes the pgbuf scan opt-in on the stats path.

Per-page dump exists but is bit-rotted: `pgbuf_dump` (11301) +
`pgbuf_dump_if_any_fixed` (11256), inside `#if defined(CUBRID_DEBUG)`
(11217-11486, never defined by CMake). Prints per-BCB
`Buf Volid Pageid Fcnt LatchMode D A F Zone Lsa consistent …` — nearly the
target schema — but no longer compiles: `bufptr->fcnt` (11346),
`bufptr->zone` + wrong stringifier (11364), `consistenet_str` typo (11368,
also 3282). Live helpers: `pgbuf_latch_mode_str` 14886, `pgbuf_zone_str`
14919, `pgbuf_consistent_str` 14952. Its `pgbuf_is_consistent` (11400)
re-reads the page from disk — very expensive.

Related: `pgbuf_bcbmon_*` BCB-mutex ownership tracking gated by
`pgbuf_Monitor_locks` (946, macros 950-957) — the gating template below.

## 3. System parameters — the ideal precedent

Adding one = four mechanical edits (rule at `system_parameter.h:92-95`):
name define, `PRM_ID_*` in enum order (`system_parameter.h:96-542`,
`PRM_LAST_ID` currently `PRM_ID_BESTSPACE_SHARD_COUNT` at :540-541),
`prm_Def[]` entry (`system_parameter.c:1018`), read via `prm_get_bool_value`.
Flags — `system_parameter.h:617-642`: `PRM_FOR_CLIENT 0x2`,
`PRM_FOR_SERVER 0x4`, `PRM_HIDDEN 0x8`, `PRM_USER_CHANGE`,
`PRM_TEST_CHANGE`, `PRM_FORCE_SERVER`, …

**The precedent: `pgbuf_monitor_locks`** — a page-buffer-specific, hidden,
server-only boolean, forced on in debug and param-gated in release:

- name `system_parameter.c:674`; enum `system_parameter.h:404`;
  entry `system_parameter.c:4180-4191` with `(PRM_FOR_SERVER | PRM_HIDDEN)`,
  default false.
- Gating idiom, `page_buffer.c:1674-1680`:

  ```c
  #if defined (SERVER_MODE)
  #if defined (NDEBUG)
    pgbuf_Monitor_locks = prm_get_bool_value (PRM_ID_PB_MONITOR_LOCKS);
  #else /* !NDEBUG */
    pgbuf_Monitor_locks = true;
  #endif
  #endif
  ```

  Read once at `pgbuf_initialize` into a file-static bool (946), branched via
  macros (951-957) — hot path pays one predictable branch, never a
  `prm_get_*` call.

Other patterns: `PRM_ID_ENABLE_MEMORY_MONITORING` (`system_parameter.h:488`,
entry 4984-4993, note `PRM_FORCE_SERVER` so a client utility sees the
server's setting); `PRM_ID_EXTENDED_STATISTICS_ACTIVATION` (:394);
`PRM_ID_PERF_TEST_MODE` (:439); graded-level variant
`PRM_ID_PB_DEBUG_PAGE_VALIDATION_LEVEL` (:176, consumed at
`page_buffer.c:10995` against `PGBUF_DEBUG_PAGE_VALIDATION_LEVEL`,
`page_buffer.h:205-211`).

## 4. Local IPC in cub_server today

**An AF_UNIX request/response control channel already exists in-tree:**
`src/connection/controller.hpp` — `template <RX,TX> class controller`:
`open(path, flags)` = `::socket(AF_UNIX, SOCK_DGRAM|flags)` (:139), asserts
`SOCK_NONBLOCK` (:122), unlink-stale + bind (:132, :145); `recv` (:170) /
`send` (:193) returning Ok/Pending/Error. Its one user is the connection-pool
coordinator: binds `"/tmp/cub_server_<pid>_coordinator.sock"` with
`SOCK_NONBLOCK | SOCK_CLOEXEC` (`coordinator.cpp:73-74`), fd in the epoll
loop (:79); POD message structs (`coordinator.hpp:140-150`); `switch`
dispatch `handle_controller_request` (`coordinator.cpp:1132-1178`).
**Gate: `#if defined (ENABLE_CONTROLLER)` — defined nowhere in CMake**;
compiled out by default. Closest existing thing to the proposed inspector;
proves the pattern is acceptable in-tree. Open questions: which thread owns
the fd (coordinator loop is pool-specific); datagram size vs a 100k-BCB pool.

**The other established channel: NET_SERVER request + `cubrid` subcommand**
(`memmon` is the freshest clone target):

| Layer | Site |
|---|---|
| Request enum | `network.h:272-274` (`NET_SERVER_MMON_GET_SERVER_INFO`, X-macro list) |
| Server dispatch | `network_sr.c:761-762` (`net_Requests[...] = smmon_get_server_info`) |
| Server handler | `network_interface_sr.cpp:12135` — checks `mmon_is_memory_monitor_enabled()` (:12146), `ER_FAILED` if off, `or_packed_*` packing |
| Client stub | `network_interface_cl.c:11629` via `net_client_request2` |
| Utility | `util_cs.c:5066` (`memmon`), `#if defined(CS_MODE)` |
| Server impl | `memory_monitor_sr.cpp` (whole file `#if !defined(WINDOWS)`) |

Also existing "server dumps diagnostics on request" precedents:
`NET_SERVER_MNT_SERVER_COPY_STATS`/START/STOP (`network.h:166-169`, statdump
via `perfmon_server_copy_stats`, `perf_monitor.c:790`);
`NET_SERVER_LOG_DUMP_STAT`, `NET_SERVER_LK_DUMP` (`network.h:126-135`).

Not there: no gRPC/protobuf/Thrift/HTTP (3rdparty/ is empty of them;
CMake options are only `ENABLE_32BIT, ENABLE_SYSTEMTAP, USE_DUMA, WITH_JDBC,
WITH_CMSERVER, UNIT_TESTS, UNIT_TEST_OOS, WITH_CCI`, `CMakeLists.txt:68-75`).
Broker/CAS AF_UNIX and shm are broker-side, not cub_server. Server↔master
datagrams (`tcp.c:881,929,966,1058`) are fd-passing, not a query channel.
`cubrid diagdb` is SA-mode/offline (`util_sa.c:1519`). Socket-path length
constraint acknowledged at `environment_variable.c:73`.

## 5. Double write buffer

`src/storage/double_write_buffer.cpp/.hpp`. Global `dwb_Global` (307);
`position_with_flags` packed cursor+state (277-280); slot record `DWB_SLOT`
with `VPID vpid` (`double_write_buffer.hpp:32-42`).

Per-VPID probe exists: lock-free hashmap keyed by VPID
(`dwb_hashmap_type`, :258; member `slots_hashmap` :279).
`dwb_read_page(thread_p, vpid, io_page, success)` (:3978) —
early-out if `!dwb_is_created()` (:3988), `slots_hashmap.find` (:3993),
VPID re-verify (:3999), memcpy page, unlock entry mutex (:4010).
Caveats: copies a full page (a residency boolean wants a lighter probe);
`find()` returns entry with mutex held and participates in the lock-free
delete protocol — needs a valid `THREAD_ENTRY`. Folding "in DWB?" into a
whole-pool sweep = N hashmap probes each taking an entry mutex — a
materially different cost profile from the lock-free BCB scan.

## 6. Gates to follow

| Gate | Kind | Evidence |
|---|---|---|
| `ENABLE_CONTROLLER` | hand-define only, wraps a whole AF_UNIX subsystem | `coordinator.hpp:121,247,376`; `coordinator.cpp:71,98,1131,1355` |
| `NDEBUG` | canonical "free in debug, param-gated in release" | `page_buffer.c:1675-1679` |
| `CUBRID_DEBUG` | legacy, never defined; guarded code has decayed | `page_buffer.c:110,…,11217-11486` |
| `SERVER_MODE`/`CS_MODE`/`SA_MODE` | binary selection; BCB mutex/waiters exist only in SERVER_MODE | `page_buffer.c:512-524` |
| whole-file `#if !defined(WINDOWS)` | optional subsystem per-platform | `memory_monitor_sr.cpp:23` |
| `ENABLE_SYSTEMTAP` | CMake `option()` → `add_definitions` — the model for cmake-visible optional diagnostics | `CMakeLists.txt:58-61,69,682-709` |
| cached-bool-from-param | `pgbuf_Monitor_locks` static + macro wrappers | `page_buffer.c:946,950-957,1674-1680` |

Background-thread pattern if needed: `pgbuf_daemons_init` (17169),
`cubthread::daemon` (1344-1347), stats slots (17202-17214).

## my-cubrid-docs prior art

No document mentions volmap, heatmap, or a page-buffer "inspector". Exists:

- `pgbuf-analysis/e6ed61e_claude/` — 11-part Korean deep-dive.
  **`06-misc-observability.md` is directly relevant**: documents
  `PGBUF_STATUS` sharding, the lock-free full scan, all 19 SHOW columns,
  `pgbuf_peek_stats`, and the per-page accessor catalog. Read before
  designing. Also `01-structures`, `02-fix-unfix-latch`,
  `04-flush-wal-daemons`, `08-page-buffer-new-plan`,
  `10-CBRD-27263-repro-proof-and-solutions`,
  `research/lockfree-fix-origin.md`, `research/prevent-dealloc-necessity.md`.
- `pgbuf-analysis/` top level (base 5cd4f860e): defects report, PG/InnoDB
  comparison report, `research/cubrid-flush-wal-dwb.md` (DWB-relevant).
- `pgbuf-rebuild-spec/` — HTML spec book (ch03 data structures, ch04
  concurrency, ch09 external contracts).
- **`cbrd-26325/CBRD-26325-instrumentation-proposal.md`** — the closest prior
  art: a four-phase escalating per-page instrumentation proposal for latch
  timeout diagnosis; the existing in-house argument if reviewers ask "why add
  per-page instrumentation".

## Cheapest viable exposure paths (observations, not a design)

1. **Per-BCB `SHOW` statement variant** — everything pre-built
   (`SHOWSTMT_*` enum near `storage_common.h:966`, `show_meta.c`,
   `show_scan.c:245-249`, scan modeled on `pgbuf_start_scan` emitting N rows
   via `showstmt_alloc_array_context`). Zero new IPC, DBA-gated. Downside:
   whole pool as one tuple array; volmap would need a SQL client.
2. **`NET_SERVER_PGBUF_*` + `cubrid` subcommand (memmon clone)** — five known
   edit sites; compact binary blob per poll. Downside: volmap must speak the
   client protocol or shell out.
3. **`ENABLE_CONTROLLER`-style AF_UNIX socket** — ready-made in-tree
   bind/recv/send with POD messages; cheapest path to volmap talking directly
   to the server with no CUBRID client protocol. Open: fd ownership thread,
   datagram sizing.
4. **Piggyback the lock-free scan** — `pgbuf_scan_bcb_table` (17257) /
   `pgbuf_peek_stats` (14684) already read flags+vpid+ptype across every BCB
   with no locks; emitting per-BCB records adds no new synchronization risk
   beyond what ships today.
5. **Resurrect `pgbuf_dump` (11301)** as the record producer — fix three
   rotted references, drop the disk-reading `pgbuf_is_consistent`.
6. **Shared memory** — no cub_server precedent; most new code; only path
   where poll frequency costs the server nothing.

Cross-cutting: the per-page payload can be a flat POD —
`(pool_index, volid, pageid, atomic_latch.raw, flags, page_lsa,
oldest_unflush_lsa)` ≈ 40 bytes; `atomic_latch.raw` is one atomic load
carrying latch+waiter+fcnt coherently. The DWB probe has a different locking
discipline and belongs to a later phase.
