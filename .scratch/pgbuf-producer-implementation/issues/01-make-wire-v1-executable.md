# 01: Make wire v1 executable

**What to build:** A maintainer can encode and validate representative v1 exchanges offline against a producer-owned, versioned contract. The executable corpus gives both repositories an exact implementation target before a live endpoint exists.

**Blocked by:** None (can start immediately).

**Status:** done

- [x] Revalidate the implementation base and create an isolated branch from the accepted Volmap format pin e1e651d, preserving unrelated work. Record the exact base. Make any small prerequisite refactoring behavior-preserving and test it before adding behavior; no broad refactor is required by this ticket.
- [x] Define exact handshake, request, scan header, record, footer and error schemas, including required fields, sequence/incarnation binding, time units, nulls, lossless numeric encodings, LSA representation and safe unknown/invalid enum handling. Include the full semantic field set and incarnation-scoped LRU topology from the producer specification.
- [x] Use snake_case fields and lowercase semantic enums, additive same-major evolution and bounded unknown-field handling. Required identity/framing/count fields remain mandatory. Map semantic page kinds on both delivery branches, including reserved oos; expose no raw PAGE_TYPE ordinals, flags, pointers, process identities or application bytes.
- [x] Specify version-unsupported, busy, rate-limited and incarnation-changed refusals. Record that disabled v1 has no listener and does not emit parameter-off; identity-mismatch is the consumer refusal reason. Specify request ordering and invalid-message behavior sufficiently for independent implementations.
- [x] Exercise reusable production encoding/validation code against independently stated expected outcomes for empty/complete/partial scans, malformed JSON, missing mandatory fields, additive fields, invalid enums, count/sequence/incarnation mismatch, duplicate-VPID ambiguity and missing-footer EOF. No live BCB source or public test endpoint is needed.
- [x] Include boundary cases for 65,536 slots/records, 64 MiB framed scans, 4 KiB record/control frames including newline, 64 KiB handshakes and nesting depth 16. Large-limit fixtures may be reproducibly generated; all byte units are binary. Describe 100 ms scan floor/traversal, 64 KiB buffering, 250 ms stall, 500 ms attachment and 2 s exchange semantics without pretending offline fixtures prove I/O deadlines.
- [x] Define complete omission only for evaluated requested VPIDs, partial gaps as unknown, duplicate observations as ambiguous, mandatory valid footers, and no cross-scan completeness. A malformed prefix cannot be salvaged as a new partial capture.
- [x] Produce a revision/content-hash manifest and reproducible offline corpus checks suitable for independent Volmap vendoring. Consumer implementation or consumer test completion does not block this ticket; matching two-repository evidence is required by ticket 06.
- [x] Use the existing Catch2 2.11.3 executable patterns and existing JSON facilities without a new runtime dependency. Record named executed cases and nonzero counts; compilation or an empty test run is insufficient.

## Source and scope

Implements the approved CUBRID producer specification for CBRD-27398 in this feature tracker. Read its full contract and linked decision authority before implementation. The public protocol is the main test boundary; focused deterministic scan/serializer checks supplement real I/O. This ticket does not add resident-page capture, AOUT or event history, native LRU telemetry, or Volmap UI work.

## Comments

2026-09-09: Published after the user approved the eight-ticket breakdown and blocking edges. Ready-for-agent describes triage readiness; blockers and acceptance criteria remain unsatisfied until evidenced.

2026-09-09 completion: implemented in commit 51531be5898e7f0634c313cc5b33ab2c792d3642 on CBRD-27398-pgbuf-inspector-contract, preserving the existing isolated e1e651d-based branch and its refusal skeleton. All nine acceptance criteria are complete. Exact frame schemas, bounded incremental validation, semantic branch mapping and corpus revision 2 (39 chronological exchanges) are executable. Independent hashes and corpus regeneration agree.

Verification: 27/27 configured CTest targets passed after the project pre-commit formatter; the inspector executed 25 Catch2 cases with 50,606 assertions. Named JUnit outcomes and a gate manifest are retained in this feature tracker's ticket-01 verification evidence. Standards and Spec reviews have zero remaining findings; numeric-domain, LRU consistency/forward-compatibility, embedded-NUL and encoder-size regressions were corrected and tested. The commit hook passed. The unrelated pre-existing dirty CCI submodule remains uncommitted.

Ticket 02 is now unblocked. Live socket/collector work, real timing measurements, independent Volmap corpus adoption and external publication remain their designated later tickets.
