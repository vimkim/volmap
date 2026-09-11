# 06: Integrate with the Volmap consumer

**What to build:** An operator uses the real Volmap consumer against the format-aligned CUBRID producer in debug and release and gets correctly identified, bounded observations across partial scans, failures and restart.

**Blocked by:** 04 — Prove overload and lifecycle isolation; 05 — Prove residency, dirty state and eviction; external prerequisite: working Volmap selected-page consumer, lifecycle and shared-demand behavior from companion tickets 02–04, with the canonical corpus vendored.

**Status:** complete

- [x] Run a reproducible integration driver with explicit producer/build/database/consumer inputs and exact revisions. Use disposable format-matched databases and the approved e1e651d-based producer; do not claim develop disk-format support from wire compatibility.
- [x] Verify the producer-owned corpus revision and content hash equal the consumer vendored copy and execute independent encoder/decoder conformance checks offline. Resolve drift in the contract or implementation, not browser-specific protocol exceptions.
- [x] Prove mutual peer and persistent-volume identity verification, copied-database refusal, version refusal and sanitized capability disclosure through the actual consumer. Runtime attachment remains explicit and loopback-only, with no automatic discovery.
- [x] Use controlled known-VPID cases to validate real semantic observations, coherent tuple meanings and incarnation-scoped LRU indices. Verify complete omission versus partial/unevaluated unknown, duplicate ambiguity, exact raw counts, malformed capture discard and no cross-scan merging.
- [x] Exercise restart and incarnation invalidation, stalled/disconnected streams, retained original age only for prior usable unexpired captures, pause/resume, hidden observers, clock changes and cancellation across shared demand. No runtime failure changes ordinary disk inspection facts, outcomes, diagnostics or revisions.
- [x] Check one shared producer connection/scan across callers, bounded admission and allocation, and consumer scheduling without multiplying traversal per tab. Use the companion consumer contract for age, expiry, scope/epoch authority and HTTP limits.
- [x] Run format-aligned integration separately with debug and release producers, recording actual executed counts, corpus identity, dataset/preconditions and raw logs. This ticket requires working consumer behavior, not completion of the companion real-producer integration or final release tickets; avoid a circular dependency.
- [x] Keep producer fixes in this workflow and consumer fixes under their companion ownership. Record shared evidence suitable for both repositories and the subsequent develop port. Full browser performance/accessibility acceptance remains ticket 08 external evidence.

## Source and scope

Implements the approved CUBRID producer specification for CBRD-27398 in this feature tracker. Read its full contract and linked decision authority before implementation. The public protocol is the main test boundary; focused deterministic scan/serializer checks supplement real I/O. This ticket does not add resident-page capture, AOUT or event history, native LRU telemetry, or Volmap UI work.

## Comments

2026-09-09: Published after the user approved the eight-ticket breakdown and blocking edges. Ready-for-agent describes triage readiness; blockers and acceptance criteria remain unsatisfied until evidenced.


2026-09-11 completion: producer 06 is complete on the format-aligned producer,
with unchanged shipped engine/corpus and a test-only permanent VPID fixture plus
actual Volmap HTTP adoption. Final debug/RelWithDebInfo each pass 11 native/HTTP
checks, 28 CTest entries, 12 real shipped-server browser cases and one freshly
executed companion CTP case (zero failures/skips). Eight actual HTTP callers
share one genuine partial producer scan. The isolated consumer source is
5dacafbb248600fd6a27b6aae8b5a1655cfa12c9; its full gate passes, including 73
frontend tests and 47 browser cases with one existing skip. Original consumer
LAN-listener changes remain untouched. Standards review has no blocking finding
(one nonblocking duplication suggestion); Spec review has no defect or scope
finding. Existing 142 evidence checksums were verified; failures and provenance
qualifications remain recorded.

Authoritative committed ledger:
`/home/vimkim/gh/cb/CBRD-27398-pgbuf-inspector-contract/docs/pgbuf-inspector/verification/06/README.md`.
Final source commit is the commit containing that ledger and
`docs/pgbuf-inspector/producer-07-handoff.md`; resolve it from git history rather
than treating the build-time base f8c068f as the final implementation commit.
Producer 07 is now the next separate-session task. Volmap 07/develop and ticket
08 release/platform/performance/manual-accessibility obligations remain open.

Final producer source commit: `3a5eb54f0282b74687b4f08684df7d0bfe9f6a0e`. No source push/PR/JIRA publication performed.
