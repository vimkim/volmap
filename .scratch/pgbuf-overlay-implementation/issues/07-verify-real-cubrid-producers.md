# 07: Verify the complete overlay against real CUBRID producers

**What to build:** Maintainers can reproduce the complete overlay against actual CUBRID builds and distinguish proven interoperability from simulated success, missing prerequisites or incompatible disk-format assumptions.

**Blocked by:** 03 — Handle pause, resume, failures, and producer restarts; 05 — Render the heatmap and LRU topology; external CBRD-27398 delivery of usable producer builds and access to its companion testcase evidence. This ticket coordinates and verifies external engine work; it does not implement that producer inside Volmap.

**Status:** ready-for-agent

**Implementation:** complete — see the final requirement audit and accepted evidence below.

- [x] Record exact producer/consumer commits and canonical corpus revision/hash. Both repositories independently run the pinned corpus offline, covering valid/partial/malformed/additive-field cases, identity/sequence/count violations, duplicate ambiguity and depth/size limits. Prove named cases executed with nonzero counts; compilation or an empty test runner is not a pass.
- [x] Establish known-VPID residency, dirty and eviction preconditions with independent bounded synchronization, not sleeps or workload timing guesses. Failed preconditions are inconclusive. Keep the observer non-invasive: no page load/copy/hash/disk-read operation, production control endpoint, or page-protection wait is added to the observation source.
- [x] Test the full attachment-to-HTTP-to-browser journey on the agreed format-aligned baseline e1e651d and the resulting producer implementation commits in both debug and release. Separately verify the develop producer port in debug and release; semantic wire compatibility does not establish Volmap develop disk-format support.
- [x] Exercise shipped, unmodified release attachment/lifecycle behavior in addition to test-only synchronization. Verify exact-UID peer authentication without root/group exceptions, directory/socket ownership, complete permanent-volume identity and copied-volume mismatch, identity-proof overflow refusal, protocol refusal, no-store and sanitized HTTP/UI output.
- [x] Obtain producer-side evidence for a private 0700 directory and 0600 socket; preservation of symlinks, wrong owners, non-sockets and active sockets; reclamation only of confirmed stale same-owner sockets; and shutdown unlinking only the exact socket created. Activation failure must not prevent database startup and must leave inspection unavailable for that incarnation without background bind retry.
- [x] Obtain producer evidence that the hidden startup-only server Boolean enable_pgbuf_inspector defaults false, creates neither daemon nor socket when disabled, has no reload/user-change/client-synchronization flags, and supports Unix server debug/release without a new v1 compile option. Windows and non-server binaries provide no endpoint. Missing sockets do not disclose the remote parameter value.
- [x] Verify producer limits below/at/above boundaries: at most 65,536 visited slots and emitted records, 64 MiB whole framed scan with footer space reserved, 4 KiB record/control frames, 64 KiB handshake, depth 16, 64 KiB output buffering, two clients and a 100 ms scan floor. Traversal/serialization checks 100 ms elapsed between slots including backpressure; stalled output disconnects after 250 ms without progress. These are not licenses to hold workers/latches for a deadline or claims of hard real-time execution.
- [x] Verify consumer 500 ms connect/handshake and two-second exchange deadlines, bounded assembly and admission, partial/full footer semantics, exact-cap full traversal, next-start rotation beyond visited span, at-most-once slot visits and larger-pool honest partial coverage. Never merge rotating captures into absence proof. Check semantic page-kind mappings and coherent latch/LRU tuples independently of raw native encodings.
- [x] Exercise simultaneous tabs, cancellation/overload, broken streams retaining only original-age evidence, refusal/explicit retry, pause/hidden/resume, restart while paused, and unaffected disk inspection. Scripted cases remain required alongside real-engine checks, not replaced by them.
- [x] Record the actual companion CUBRID external shell testcase revision and execution evidence using project-supported test concepts. Private-suite or credential-isolation access missing from the environment is missing evidence, never an implicit pass. Do not present personal convenience tooling as organization workflow.
- [x] Produce reproducible commands, preconditions, named case counts, build/environment details, status and raw artifacts for each cross-repo invariant. Missing, skipped, failed or inconclusive checks leave integration open, and code/corpus changes require affected checks to rerun.

## Comments

2026-09-11 — Implemented an explicit-input real-producer Playwright driver and
recorded 12 passing cases in each format-aligned debug/release run (Chromium and
Firefox, zero failures/skips). Actual attachment, copied-volume refusal,
sanitized no-store responses, paused restart/retry, both-scale rendering and
independent tab pause/resume are covered. Producer native, credential,
shipped-server and historical external shell evidence remain separately labelled.
The final local verification gate passed after repairing an existing Firefox
navigation test; the earlier HTTP admission failure remains unresolved.

Integration is still incomplete: the develop producer delivery and execution,
full platform/build evidence and the remaining cross-repo invariant qualifications
are open. See the [requirement ledger and raw evidence](../verification/07-real-journey/README.md)
and [reproduction instructions](../../../docs/runtime-overlay-verification.md).
No release completion is claimed.

2026-09-11 — Follow-up reproduced the historical HTTP admission failure and
identified an intrusive polling-request race in the test. The repaired test
requires one refusal among nine valid concurrent callers and preserves all eight
successful caller checks. It passed 500 consecutive focused executions. See
[the causal diagnosis and raw evidence](../verification/07-http-admission/README.md).
This supersedes the unresolved HTTP-test qualification above; the external
producer and integration gaps remain open.
The full `just verify` gate also passed after this repair (47 browser passes, one existing skip).

2026-09-11 — Resumed work item 126 from producer `48a3e87e3` handoff.
Develop source/build/installation hashes and both producer07 and retained
producer06 evidence sets revalidate. Fresh independent corpus executions pass
on both develop builds and current consumer `7c6e7e9`; full local verification
passes (74 frontend unit tests, 65 browser passes, one existing skip). Fresh
format-aligned Debug and RelWithDebInfo browser runs each pass 12 cases; retain
the first release attempt's transient bind failure separately. Producer06 now
supplies controlled permanent-VPID evidence through actual HTTP at consumer
`5dacafb`; it is not controlled-state browser evidence.

The external develop producer is delivered, superseding the earlier missing
producer note. Independent develop disk corpus and source-layout compatibility
remain unproven, so develop browser integration and ticket completion stay open.
See [the resumed ledger](../verification/07-develop/README.md) for exact inputs,
counts, raw evidence, failed attempt and remaining platform qualifications.

2026-09-11 — Completed work item 126. Independent develop disk fixtures exposed
and now guard the 1152/1160-byte heap layout and 200/216-byte file-header
boundary. Six new public-decoder cases and the existing aligned corpus pass.
Final real browser verification passes all 48 cases across develop/aligned and
Debug/RelWithDebInfo; controlled permanent native-to-HTTP checks pass 11 lines
per mode. The full local gate passes, including the preserved exact-UID check.

[The final requirement audit](../verification/07-develop-disk/completion-audit.md)
maps all eleven criteria to named evidence. Independent Standards and Spec
reviews report zero findings and support ticket completion. CUBRID's official
release mode maps to RelWithDebInfo. Windows exclusion is proven at the source
and build-selection boundary, with no claim of Windows execution. Prior
missing-develop and missing-disk-corpus notes are superseded; failed attempts
remain retained. Performance, manual accessibility and whole-feature release
readiness remain the separate ticket 08 gates. No publication was performed.
