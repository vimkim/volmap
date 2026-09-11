# Ticket 07: shipped-server and credential follow-up

Status: incomplete integration; additional producer evidence recorded on
2026-09-11. This supplements, rather than replaces, the earlier
[preflight](../07-real-producers/README.md).

The producer is still `f8c068f771ccd141a3c2f08541fe75c84e41ce53`, descended
from the agreed `e1e651d` format baseline. Consumer changes in this follow-up
start at `15c3b58`. The manifest records exact binary/helper hashes and runnable
commands, including explicit debug/release installations. No engine or external
testcase source was changed.

## Newly executed producer checks

Both debug and release ran the existing `server_attachment.py --scan --isolation`
driver against the installed, unmodified `cub_server`. Each run emitted 11
passing checks, including three complete resident scans with nonzero counts.
Each checks the complete persistent-volume set, including the extension volume,
and the private directory/socket modes. It also checks restart incarnation,
copied-database identity difference, replacement-socket preservation at shutdown,
activation failure preserving the conflicting entry and SQL service, and absence
of background activation retry after the conflict is removed.

The driver's line labelled `default-off` actually calls `start(False)` and writes
an explicit `enable_pgbuf_inspector=no`. It proves explicitly disabled startup,
not the omitted-parameter default. Separately, the pinned
`src/base/system_parameter.c:4200` declaration uses `PRM_FOR_SERVER | PRM_HIDDEN`,
`PRM_BOOLEAN`, `PRM_CLEAR_DYNAMIC_FLAG`, and false defaults. The pinned
`src/storage/pgbuf_inspector.cpp:113` returns before identity collection/task
creation when disabled. These are source evidence, not a fresh omitted-parameter
or Windows/non-server execution result.

Independent Linux socket diagnostics confirmed two server send queues were
saturated before and after a SQL update/commit/readback. Debug observed 13 busy
refusals during a 0.195-second overlap, and release observed five during a
0.153-second overlap. `server-MODE/isolation.json` retains both queue measurements;
worker logs retain the committed result. These are actual overlapping workloads,
not a performance acceptance claim or a hard scheduling guarantee.

The separately selected `[.credential]` tests ran under
`unshare --map-auto --map-root-user` in both modes. Each executed four cases and
41 assertions: different effective UID refusal, no root exception, preservation
of a wrong-owner socket, and preservation of a wrong-owner private directory.
The different-UID client retains the server group, covering absence of a group
exception. XML reports retain all case names and zero failed assertions.

## Companion shell evidence

The available companion testcase is
`a648d78f599504fe0c916628ea51ea3f2b5f7ca7`, path
`shell/_06_issues/_26_2h/cbrd_27398/cases/cbrd_27398.sh`.
The retained September 9 CTP logs show one executed, one successful, zero failed,
and zero skipped cases for each mode. The case's child identity logs record both
repository commits, the actual shared-library resolution, and hashes of the
server, library, native fixture and both producer scripts. All five hashes were
checked against the corresponding current local files and matched in each mode.

`previous-ctp-MODE/` and `previous-ctp-MODE.log.gz` preserve that prior evidence.
These are audited historical runs, not newly executed CTP results. The project
runner was `ctp.sh shell -c CONF`, in isolated namespaces with a copied
installation/registry and testcase updates disabled. The original transcripts
retain the concrete invocation and effective case count. This does not prove
consumer integration, a develop port, or unrelated private-suite coverage.

## Consumer regression repair

The existing Firefox lifecycle case failed again when run alone. Its trace shows
the second page's document, scripts and API requests completed with HTTP 200,
but `goto(..., waitUntil: "commit")` did not return. The failure coincided with
outstanding live-watch requests; the browser/framework root cause is not proven.
The same test now navigates through the document and uses its existing bounded
UI assertions to establish readiness. Both Chromium and Firefox then passed the
case, retaining checks for another-tab newer offers, pause, hidden silence,
resume and expiry. No application logic or timeout was changed.

The selected-page test title now identifies its scripted socket producer. Its
HTTP server is real, but its Python fixture is not CUBRID; a passing browser
report must not suggest otherwise. Typechecking passed. `firefox-red-trace.tar.gz`
preserves the failing trace, and `navigation-green.log.gz` preserves both passing
browser cases. The final `just verify` passed, including 47 browser cases with
one pre-existing skip, frontend types/unit tests, Rust tests, Clippy and static
musl checks. Full output is recorded in `verify.log.gz`. This pass does not erase
the earlier HTTP admission failure, whose cause has not yet been resolved.

## Remaining gates

Actual producer-to-consumer HTTP/browser integration has not run. The ticket
already specifies that test boundary; the earlier request to reconfirm it was
unnecessary. The next integration driver will use that boundary. No local develop producer port was
found, and this Volmap ticket does not implement that external engine delivery.
The native controlled page is a temporary-volume VPID (`32766:65`); those
producer-only results do not establish selected-page rendering for an inspected
permanent volume. Full build/platform gating, cross-repo invariant coverage and
the unresolved first-run HTTP admission failure also remain open. Do not mark
ticket 07 complete from this evidence.

Read compressed logs with `gzip -dc FILE.log.gz`. Verify retained artifacts
from this directory with `sha256sum -c SHA256SUMS`.
