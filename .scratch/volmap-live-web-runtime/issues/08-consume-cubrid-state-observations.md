# 08: Consume CUBRID state-only page-buffer observations

**What to build:** Implement W7's protected UDS consumer after work-tracker item 27 publishes its reviewed CUBRID producer handoff. Normalize the finalized wire into Volmap runtime terms before it reaches HTTP or React.

**Blocked by:** 06: Add the loopback runtime broker and HTTP boundary; completed and reviewed producer handoff from work-tracker item 27.

**Status:** ready-for-agent

- [ ] The exact producer protocol/version, CUBRID landing branch, compile/runtime gates, socket permissions, peer policy, limits, and test fixtures come only from item 27's final handoff.
- [ ] The handshake proves database and volume identity plus one producer incarnation before observations become active.
- [ ] State-only batch requests do not load missing pages, copy/hash page images, perform disk I/O, or wait without producer/Volmap deadlines.
- [ ] Producer semantics normalize into residency, fixed state, latch state, dirty, flushing, page LSA, capture token, and limitations without exposing raw enum ordinals, flags, pointers, holders, or private structures.
- [ ] Page kind and every producer enum cross the seam as reviewed semantic vocabulary, never C/C++ numeric layout.
- [ ] Restart/incarnation change atomically clears producer-derived state, resident structure, correspondence, and in-flight adoption authority while preserving the disk route.
- [ ] Identity mismatch, gate disabled, peer refusal, protocol mismatch, deadline, partial capture, and over-budget behavior map to distinct Volmap capability/coverage states.
- [ ] Simulated adapter tests and exact real-producer contract tests use the same Volmap adapter interface and prove bounded, per-page, non-atomic semantics.
- [ ] HTTP, DOM, log, and snapshot sentinels prove structural-only output and absence of raw/in-memory application data.
