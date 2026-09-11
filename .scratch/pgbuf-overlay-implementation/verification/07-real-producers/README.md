# Ticket 07: real-producer verification preflight

Status: incomplete. This is an execution checkpoint, not an integration pass.
Recorded 2026-09-11. No implementation or release completion is claimed.

Consumer: `f8f315c9c18d3707ad19b0405ee06afb34c79549`.
Producer: `f8c068f771ccd141a3c2f08541fe75c84e41ce53`, descended from
`e1e651debf6cc100172bde96603b17424f9c135a`. Existing CCI submodule changes
and an untracked producer log were preserved. Existing installed binaries were
used; this run did not rebuild or independently establish their source provenance.

[preflight.json](preflight.json) records commands, binary hashes, environment,
named JUnit leaf cases, native fixture results, and retained artifact hashes.
Its commands use absolute paths to identify the actual local inputs. Consumer
`just` commands are Volmap local verification recipes, not CUBRID organization
workflow instructions.

## Executed evidence

The producer and consumer v1 directories match byte-for-byte across 109 files.
Corpus revision 2 has SHA-256
`11dbecc72e4b9dd78e22080f138c801c189c7e047c0db23af8233f44a804d5ce`.
The Rust pin test passed, and 28 observation-module tests passed, including the
decoder test which asserts execution of all 39 canonical exchange cases.

Both existing producer test binaries passed with 87 named JUnit leaf entries
each and no reported failures/errors/skips. Catch2's suite `tests` attribute
counts assertions; it must not be presented as the number of test cases.
These runs do not include opt-in credential-isolation cases.

Each build's native fixture passed six named checks: clean held page, dirty held
page, partial capture of an independently held page, evicted complete omission,
evicted partial omission, and shutdown. The VPID was `32766:65`. The fixture's
bounded command acknowledgements establish state before and after observation;
timing delays are not residency or dirty-state oracles. Complete omission was
reported as observed nonresident, and partial omission remained unknown.
The `native-debug/` and `native-release/` directories retain synchronization,
wire captures, conclusions and process logs. These are test-only native owners
of the production observation daemon, not the shipped `cub_server` lifecycle.

The first `just verify` failed in the existing
`concurrent_http_scopes_share_scan_but_keep_coverage_and_disk_admission_independent`
test: eight HTTP waiters did not occupy admission before its deadline. The exact
test passed alone. Preserve `verify.log.gz` and `http-admission-recheck.log.gz`; a later
pass cannot erase this unresolved failure. See `verify-recheck.log.gz` for the
subsequent complete-gate attempt.

The recheck passed Rust tests, Clippy, the static-musl artifact check and frontend
unit/type checks, but failed Firefox's other-tab navigation at
`web/e2e/observations.spec.ts:58` after 30 seconds. Browser totals were 46 passed,
one failed and one skipped. `browser-failure.tar.gz` retains the failure trace;
`scripted-browser-screenshots.tar.gz` retains newly captured scripted screenshots
without replacing historical evidence. Neither full-gate attempt passed.

Independent Standards and Spec reviews found no inaccurate pass claims,
documented-standard violations or scope creep in this checkpoint. The Spec
review confirmed the unfinished gates below. These reviews cover this evidence
addition, not completion of ticket 07.

## Open requirements

| Ticket requirement | Current evidence boundary |
| --- | --- |
| Exact revisions and offline corpus | Consumer pin/decoder and producer unit binaries ran; exact build-source attestation remains open. |
| Independently synchronized known VPID | Debug/release native fixture ran; actual consumer adoption of these states remains open. |
| Attachment → HTTP → browser on both producer branches/builds | Not executed. No local develop producer port was identified. |
| Shipped release lifecycle, exact UID, identity/refusal, sanitized HTTP/UI | Ordinary producer unit cases ran; fresh installed-server, credential-isolation and integrated consumer checks remain open. |
| Namespace ownership and lifecycle | Ordinary socket unit cases ran; shipped-server and credential-isolation coverage remains open. |
| Default-off/startup-only and platform/build matrix | Prior producer tickets contain evidence; not revalidated or promoted to a current integration pass. |
| Producer budget boundaries | Named producer unit cases ran; full cross-repo invariant mapping and shipped-build attribution remain open. |
| Consumer deadlines, coverage, tuples | Existing 28 observation tests ran; real-engine cross-repo coverage remains open. |
| Tabs, cancellation, failures, pause, restart, disk independence | Existing scripted full-suite attempt failed in HTTP admission; no real-engine browser result yet. |
| Companion external shell testcase | Prior ticket 05 records revision `a648d78f599504fe0c916628ea51ea3f2b5f7ca7`; no fresh execution or access verification in this checkpoint. |
| Complete reproducible invariant evidence | Incomplete; no unchecked requirement becomes a pass. |

The current ADR 0007 supports explicit persistent format profiles, including a
pinned develop profile. This postdates the ticket's older disk-format warning.
The producer baseline above requires the `feat-oos` profile; semantic wire
compatibility alone still proves nothing about either persistent format profile.

## Resume

Confirm the proposed new test boundary: actual producer socket through Volmap
HTTP through Chromium/Firefox, with independent producer fixture acknowledgements
for controlled state. Then implement and run that integration driver against
explicit disposable inputs. Obtain the develop port and its matching debug and
release builds from the producer workflow; this Volmap ticket does not port the
engine. Revalidate affected evidence after source/corpus changes. Ticket 07
remains open until every required invariant has nonzero, passing evidence.

Raw command logs use lossless gzip compression; read them with `gzip -dc FILE.log.gz`.
Verify retained bytes with `sha256sum -c SHA256SUMS` from this directory.
