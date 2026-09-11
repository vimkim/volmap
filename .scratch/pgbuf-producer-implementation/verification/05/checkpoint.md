# Ticket 05 completed

Engine commit: `f8c068f771ccd141a3c2f08541fe75c84e41ce53`. Reviewed tree: `890d5c4c6e505ef93f55abb86d66bfe72e136420`.
External testcase commit: `a648d78f599504fe0c916628ea51ea3f2b5f7ca7`. Reviewed tree: `5b1f7792534f65b85b09fd4de21afe9a9badd5ea`.
External worktree: `/home/vimkim/gh/tc/CBRD-27398-pgbuf-inspector-fixtures`.
Case: `shell/_06_issues/_26_2h/cbrd_27398/cases/cbrd_27398.sh`.

Debug and RelWithDebInfo each pass all 28 configured CTest entries, including
61 inspector cases and six native semantic/lifecycle checks. Actual external
CTP executes one companion case per mode: success 1, failure 0, skip 0. Each
companion independently runs the unmodified installed cub_server for default-off,
attachment, persistent identity and restart checks. Both review axes have zero
unresolved findings.

| Acceptance | Evidence |
| --- | --- |
| Known VPID and independently held clean/dirty state | Native allocation, flush/set-dirty, retained WRITE fix and before/after native acknowledgements; clean/dirty JSONL |
| Actual removal and no refix | 4096-page capacity workload in each final run; native cache-only target miss before workload cleanup; still-allocated private target excluded from cleanup; four absence acknowledgements |
| Complete absence versus partial unknown | evicted-complete/evicted-partial JSONL and conclusion JSON; independently rechecked by collect_final.py |
| Shipped producer, no observer page operations or production hook | Production boot/daemon and linked installed libcubrid; Python observer uses real socket; source diff touches only tests/docs |
| Actual external shell suite and standard invocation | Exact testcase commit, README CTP command, named case and positive execution summaries in ctp-*-final.log |
| Debug/Release and separate unmodified Release checks | Full CTest logs plus ctp-*-final-case/controlled.log and attachment.log |
| Revisions, bounds, named counts and raw artifacts | manifest.json with commits, reviewed trees, per-binary hashes, build modes, six named cases per mode and retained raw artifacts |

The native fixture bounds replacement to 32768 allocations/fifteen seconds and
non-target cleanup to five seconds. Cleanup may retry a legal invalidation no-op,
but only a cache-only miss establishes removal. The target eviction proof comes
first. The stdin command deadline remains ten seconds, with bounded controller
acknowledgements. Neither sleep nor inspector output is a page-state oracle.

Failed attempts remain recorded: incorrect initial temporary-file destruction,
a Debug-only native helper, a fixed-size slow drain that failed to release a
kernel send allocation, output pressure preventing a complete eviction capture,
and a legal invalidation no-op during Release cleanup. Final code uses normal
temporary-file retirement, Release-compatible native checks, proven write
progress, target-excluding workload cleanup, and bounded cleanup retries.

See [manifest.json](manifest.json) for machine-checked evidence and
[review.md](review.md) for the two independent review reports. Earlier
progress.json/source snapshots are historical, not the final gate.
The source preset is debug_gcc, the external testcase worktree is clean, and the
preexisting dirty cubrid-cci submodule is preserved. Both commits are local;
no publication or consumer/develop/performance gate is claimed. These reusable
fixtures and standard CTP instructions are available for ticket 07.
