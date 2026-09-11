# Ticket 07: actual attachment, HTTP and browser journey

Integration status: **incomplete**. The format-aligned browser matrix passed;
missing external and cross-repo evidence still prevents closing ticket 07.
Recorded 2026-09-11.

The reusable [run instructions](../../../../docs/runtime-overlay-verification.md)
describe explicit inputs and evidence limits. Producer commit
`f8c068f771ccd141a3c2f08541fe75c84e41ce53` is based on
`e1e651debf6cc100172bde96603b17424f9c135a`. Both runs use `feat-oos` and
independently created disposable databases. The tested consumer source base is
`646e382`; application source was unchanged while this harness was added.
Each runtime JSON records the source dirty state, actual binary hashes, build
type and exact harness hashes. This records input identity, not a new build
attestation for the external installation.

## Accepted executions

| Producer / consumer | Browser cases | Result |
| --- | --- | --- |
| Debug / debug static-musl Volmap | 6 Chromium + 6 Firefox | 12 passed, zero failed/skipped/flaky |
| RelWithDebInfo / release static-musl Volmap | 6 Chromium + 6 Firefox | 12 passed, zero failed/skipped/flaky |
| Scripted regression suite in final `just verify` | Chromium + Firefox | 47 passed, one existing skip; complete local gate passed |

`manifest.json` lists each real case and its browser. The adjacent
`debug-accepted-browser-results.json.gz` and `release-accepted-browser-results.json.gz`
are Playwright JSON reports, with observation JSON and screenshot attachments
encoded in the reports. Selected screenshots are also materialized as
[Chromium selected-page](release-selected-chromium.png) and
[Firefox Sector LRU](release-sector-firefox.png).

Actual installed servers supplied observations throughout these cases; no
HTTP route interception or scripted wire peer was used. Each run proved omitted
parameter startup could serve SQL without an inspector directory, then restarted
with the parameter enabled. The original volumes attached successfully; copied
permanent volumes had distinct inodes and produced `identity-mismatch`, while
disk navigation worked. Both browsers observed incarnation invalidation while
paused and recovered only after explicit retry. Volume/Sector state marks,
switching to LRU presentation, and another tab's independent pause/resume worked.

The LRU cases are rendering smoke checks, not independent native tuple proofs.
The startup page is not independently held by a native acknowledgement. Keep the
producer's controlled-state and semantic sampling evidence separate.

## Requirement ledger

| Ticket requirement | Evidence and remaining scope |
| --- | --- |
| Revisions and independent offline corpus | Revision 2, hash `11dbecc72e4b9dd78e22080f138c801c189c7e047c0db23af8233f44a804d5ce`; all 109 source/consumer files match in both real runs. [Preflight](../07-real-producers/README.md) records 87 producer JUnit leaf cases per mode and 28 consumer observation tests, including all 39 exchange cases. Build attestation remains separately qualified. |
| Known VPID, dirty and eviction synchronization | Preflight retains six native checks per mode and bounded acknowledgements for temporary VPID `32766:65`. These do not prove controlled permanent-page state through the viewer. |
| Format-aligned and develop debug/release journeys | The format-aligned 24 browser cases passed. No develop producer worktree/build delivery was found; no develop result is claimed. |
| Shipped lifecycle, credentials, identity, refusal, disclosure | This matrix proves actual attachment, copied-volume refusal, no-store, sanitized HTTP/UI, and restart. [Follow-up](../07-real-producers-followup/README.md) adds 11 shipped-server checks and four isolated credential cases per mode. Protocol/identity-overflow refusal remains focused unit/corpus evidence, not an extra shipped-browser experiment. |
| Namespace ownership and activation failure | Follow-up verifies modes, replacement preservation, SQL during overload, conflicting-entry preservation and no bind retry. Preflight socket cases include symlinks, stale/active sockets and backlog ambiguity; credential runs cover wrong owners. This does not claim every filesystem case through the browser. |
| Default-off and build/platform gating | Fresh omitted-parameter SQL startup with no directory passed in each run. Follow-up records startup-only Boolean source flags. Windows/non-server execution/build evidence remains absent. |
| Producer budget boundaries | Preflight named cases cover frame/depth/whole-scan/record/slot limits, footer reservation, client admission, output buffering, scan floor, elapsed/backpressure and write-stall deadlines. Shipped overload checks are supplemental. Develop repetition remains missing. |
| Consumer deadlines, assembly, coverage, semantic tuples | Preflight consumer cases cover actual I/O deadlines, bounded owners/response bytes, duplicate ambiguity, footer publication, rotating partial captures and no merged absence proof. Producer cases cover coherent sample decoding and native page-kind vocabulary. Real renderer smoke checks do not independently validate native tuple correctness. |
| Tabs, cancellation, failures, pause/hidden/resume | Real matrix covers multiple tabs, pause/resume and paused restart. Scripted tests remain required and ran in `just verify`; cancellation, partial/malformed streams and hidden-state cases are not relabelled as real-engine coverage. The initial HTTP admission test failure remains unresolved despite subsequent passing runs. |
| Companion external shell testcase | Follow-up preserves audited September 9 debug/release runs: testcase `a648d78f599504fe0c916628ea51ea3f2b5f7ca7`, one executed/pass per mode, no skips. Child binary/helper hashes match current files. These are historical runs, not newly executed CTP results. |
| Complete raw invariant evidence | Commands, nonzero named counts, runtime metadata and raw results are retained. The qualifications above remain open; this ledger is not a release approval. |

## Retained failed attempts

- `debug-red.log.gz`: requested port was already in use; no fixture started.
- `debug-unattached-red.log.gz`: expected red test with runtime flags omitted.
  Its original process-group teardown killed the master first and left a server;
  only that verified private-root server was terminated. Separate child sessions
  fixed normal ordered teardown; accepted logs show successful server/master stop.
- `debug-lifecycle.log.gz`: the startup daemon inherited a pipe, so the Node
  startup command timed out waiting for EOF. File-backed command output fixed the
  driver; later actual lifecycle cases passed in both browsers.
- Earlier preflight/follow-up directories retain the HTTP admission and Firefox
  navigation failures. The Firefox repair passed focused and full suites; no
  causal repair is claimed for the first HTTP admission failure.

Independent Standards and Spec reviews found no blocking implementation defect
or false pass claim. The Spec review explicitly required the LRU smoke-check
qualification retained above. `verify.log.gz` contains the passing final local
gate. Read raw logs with `gzip -dc FILE.log.gz`; check retained bytes with
`sha256sum -c SHA256SUMS` from this directory.
