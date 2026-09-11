# Ticket 07 requirement audit

This audits the eleven checkboxes of the Volmap ticket, not producer08 or
Volmap08 release readiness. The new changes affect persistent decoding and test
workspace placement. Producer, semantic wire corpus and consumer wire decoder
sources did not change; their independently executed, hash-bound evidence from
the preceding checkpoint remains applicable.

| Requirement | Authoritative evidence | Scope / limit |
| --- | --- | --- |
| Exact revisions and independent canonical corpus | Previous `07-develop/manifest.json`: 109 matching files, v1.0/revision 2/hash 11dbecc…; producer five top-level/seven JUnit corpus entries in each mode, consumer pin case and 28 observation cases including 39 exchanges. Current manifest binds changed consumer source hashes and actual binaries. | Semantic corpus and new disk corpus are independently identified; neither substitutes for the other. |
| Known VPID residency, dirty and eviction | Fresh `native-debug/release` logs and raw/HTTP archives: 11 PASS lines per mode, permanent VPID 1:577, native acknowledgements retaining WRITE fix for clean/dirty and cache miss after replacement; direct HTTP resident/nonresident assertions. | Controller belongs to the native testcase. No observer page read/fix/copy/hash or production control was added. Relay-paced partial omission remains distinct from direct observations. |
| Aligned full journey and separate develop debug/release | Four accepted browser reports: 12 Chromium/Firefox cases each, zero failures/skips/flaky cases; producer Debug and RelWithDebInfo with matching debug/release consumer binaries. Develop also has independent 19-page disk corpus, six public-decoder tests, native source-layout audit, full-dataset count agreement and selected deep inspection. | RelWithDebInfo is the executed optimized producer configuration. No plain Release/OptDebug execution is claimed. |
| Shipped lifecycle, exact UID, identity/refusal, no-store/sanitization | Fresh four-browser matrix uses installed unmodified servers, default-off SQL startup, copied volumes, restart/retry and sanitized no-store responses. Hash-validated producer07 supplies four mapped-UID cases and 11 shipped lifecycle PASS lines per mode. Consumer exact-UID test executes in accepted full gate; protocol and oversized identity refusals remain actual socket/corpus tests. | Required layers are identified; not every protocol failure is relabelled as a browser experiment. |
| Private endpoint ownership and activation failure | Hash-bound producer07 native socket/security suite and unmodified server driver: 0700/0600, wrong owner/symlink/non-socket/active/stale cases, replacement preservation, no bind retry and continued SQL on activation failure. Previous checkpoint revalidates all 151 artifact checksums. | Existing producer source is unchanged; no new execution is invented. |
| Startup parameter and build/platform exclusion | Producer07 source audit pins default false, PRM_FOR_SERVER|PRM_HIDDEN only, one startup read, no feature option. Fresh browser omitted-parameter startup checks run in both branches/modes; native createdb with parameter enabled produces no endpoint. Fresh `binary-gating-recheck.json`: inspector symbols in server libraries and zero in standalone/client libraries for both develop modes. CMake includes the endpoint only for Linux; declarations and boot calls require SERVER_MODE and LINUX. | Positive runtime evidence is Linux x86-64. Windows exclusion is source/build-selection proof, not Windows execution. No build-type/NDEBUG feature gate exists. |
| Producer budget boundaries | Producer07 registered 61-case suite per mode, named mapping in its isolation guide and retained JUnit/raw logs: visited/emitted caps, framed bytes/footer reservation, frame/handshake/depth/output limits, clients/floor, elapsed scan checks and stalled writes. | Deterministic boundaries plus actual socket deadlines; not hard real-time or performance claims. |
| Consumer deadlines, bounded assembly, coverage and semantic tuples | Accepted full gate executes 28 observation tests, including real I/O deadlines, raw size/count limits, strict footer semantics, duplicates and rotation. Producer07 native kind/packed tuple tests are independent of consumer display encodings; fresh native HTTP partial omission shares one scan across eight epochs. | Rotating partial scans are never merged into absence proof. Browser LRU remains rendering smoke coverage. |
| Tabs, cancellation, failures, pause/hidden/restart, disk independence | Four real matrices cover tabs, independent pause/resume, paused restart/retry and usable copied-volume disk navigation. Accepted full gate supplies scripted hidden-state, malformed/partial response, delayed epoch/generation, overload and age tests. Native HTTP checks preserve disk session equality. | Real and scripted checks remain separately labelled. |
| External shell testcase revision and execution | Delivered producer07 records CTP a648d78f599504fe0c916628ea51ea3f2b5f7ca7, one executed/success per mode, zero failed/skipped, child engine/helper hashes and namespace-isolated commands. These artifacts were independently checksum-verified at the preceding checkpoint. | Delivered executions, not new CTP reruns. Public reproduction uses ctest and project test concepts, not personal wrappers. |
| Reproduction and complete evidence labelling | This directory's manifest, commands/configs, corpus generator, raw reports, source/native-layout evidence and prior ledgers bind inputs/results. Failed generation, decoder tests, compilation, insufficient space and private-parent credential attempt remain explicitly retained. | Accepted reruns supersede failed attempts without deleting them. Full gate's one Firefox parity skip is deliberately owned by Chromium and is unrelated to producer/runtime invariants. |

## Completion boundary

CUBRID `build.sh:296-297` and `CMakePresets.json:21-25` explicitly map the
project's `release` mode to `RelWithDebInfo`; `producer-build-selection.txt.gz`
retains those exact source lines and file hashes. The executed optimized mode
therefore satisfies the requested release build rather than substituting a
different gate.

The ticket asks for separate real producer debug/release verification and
source/build exclusion of Windows/non-server endpoints. It does not require a
Windows execution matrix, all-project CUBRID unit build, dedicated-host p95
measurements or manual accessibility review. Earlier checkpoint notes listed
those unexecuted configurations alongside actual missing integration evidence;
this audit distinguishes their limits from this ticket's eleven requirements.
The source-level negative endpoint evidence must never be described as a
successful Windows run. Other Unix authentication and plain Release/OptDebug
execution remain outside the empirically verified matrix.

Producer08/Volmap08 keep the broader release gates: performance, manual
accessibility and any additional delivery requirements. This audit does not
publish, approve upstream integration or declare whole-feature release readiness.
Independent Spec review validated all eleven criteria and this completion
boundary with zero actionable findings; see [review.md](review.md).
