# 04: Prove overload and lifecycle isolation

**What to build:** An operator can keep database work running while hostile, excessive or stalled observation clients are refused or disconnected, with correct shutdown and restart behavior verified through real sockets.

**Blocked by:** 03 — Observe a bounded resident-set scan.

**Status:** completed

- [x] Exercise two admitted clients plus excess connection attempts, scan requests below/at/above the 100 ms floor, malformed/oversized/deep requests, fragmented and coalesced input, wrong versions/incarnations, abrupt EOF and missing-footer exchanges through production transport handling.
- [x] Prove the 64 KiB output bound, 250 ms no-progress disconnection, 500 ms handshake deadline and 2 s whole-exchange deadline with actual I/O as well as deterministic clocks. Exercise every framing/scan cap below/at/above its bound and confirm footer reservation under backpressure.
- [x] Demonstrate ordinary database worker progress during stalled clients and admission overload, bounded cancellation after disconnect, and shutdown completion without waiting on page protection or leaving producer resources alive. Fix any violations without relaxing accepted limits.
- [x] In an isolated credential environment, verify exact-UID refusal before data exchange, including absence of root/group exceptions. Exercise owner/permission checks, symlink/non-socket/wrong-owner/active-socket preservation, confirmed stale recovery and indeterminate-failure preservation.
- [x] Replace the socket pathname during the server lifetime and prove cleanup does not unlink the replacement. Activation failure must leave database startup successful with no background retry. Restart must issue a new unpredictable incarnation and terminate old exchanges.
- [x] Validate sanitized protocol output and distinguish unavailable attachment from a proven parameter setting. Broken or malformed captures must never manufacture complete or partial success.
- [x] Run real debug/release socket tests, preserve reproducible logs and executed counts, and record requirements/results in the gate evidence. Missing credential isolation or unestablished timing preconditions are missing/inconclusive evidence, not passing. This ticket can proceed independently of ticket 05.

## Source and scope

Implements the approved CUBRID producer specification for CBRD-27398 in this feature tracker. Read its full contract and linked decision authority before implementation. The public protocol is the main test boundary; focused deterministic scan/serializer checks supplement real I/O. This ticket does not add resident-page capture, AOUT or event history, native LRU telemetry, or Volmap UI work.

## Comments

2026-09-09: Published after the user approved the eight-ticket breakdown and blocking edges. Ready-for-agent describes triage readiness; blockers and acceptance criteria remain unsatisfied until evidenced.

2026-09-09 completion: committed as `e5f3cdf86908b8d23d4e09d28d22265bf240f7ad`. Debug and RelWithDebInfo each passed 27/27 CTest entries, 61 inspector cases (28 socket cases) and 4 separately isolated credential cases. Kernel diagnostics establish both current scan outputs saturated before and after committed SQL; 14 debug and 6 release excess connections received busy. Standards and Spec reviews have zero remaining findings after scoped FD cleanup and saturation-proof improvements. Failed/inconclusive preconditions and test-environment attempts remain recorded. Evidence: [manifest](../verification/04/manifest.json), [checkpoint](../verification/04/checkpoint.md). Native no-wait behavior is supported by the audited trylock/unlock path, deterministic sampling/cancellation and real SQL/shutdown; no native held-mutex fixture or later release-performance gate is claimed.
