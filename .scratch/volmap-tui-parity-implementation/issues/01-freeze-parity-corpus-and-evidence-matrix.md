# 01: Freeze the parity corpus and evidence matrix

**What to build:** Establish the checked-in evidence vocabulary that every Atlas implementation ticket will use. Follow D0 in the [implementation specification](../../volmap-tui-web-parity/implementation-spec.md) and the complete [TUI parity verification contract](../../volmap-tui-web-parity/issues/08-define-parity-verification-contract.md). This is a prefactor: it makes later behavior changes provable without changing production behavior itself.

**Blocked by:** None (can start immediately).

**Status:** superseded

**Superseded by:** [Volmap focused TUI implementation specification](../../volmap-tui-focused-inspector/implementation-spec.md).

- [ ] A checked-in parity matrix maps every accepted requirement from the first seven Wayfinder decisions to a stable `PAR-*` gate, canonical fixture, production interface, expected invariant, and blocking job.
- [ ] One deterministic exact-revision corpus covers every required allocation, physical type, occupancy, finding, attribution, Page geometry, Slot state, diagnostic, coverage, outcome, lifecycle, invalidation, and disclosure case named by the verification contract.
- [ ] Lazy fixtures cover the 33,554,432-Sector topology, exhaustive 257-Sector traversal, the real sparse-volume integration case, maximum Slot/Distribution rows, hostile terminal text, and unique disclosure sentinels without checked-in giant artifacts.
- [ ] Repository-owned state-trace generation and deterministic reduction can replay a printed seed and minimized event sequence without adding a generic property-test framework.
- [ ] Renderer golden tooling writes review candidates outside checked-in goldens, cannot bless in place, and stores normalized semantic cells and the matching `LayoutCommit` together.
- [ ] The isolated Expect and Playwright/Chromium test-tool locks, provenance, and blocking-job ownership are defined without entering the production Cargo graph or release bundle.
- [ ] The complete merge-base test inventory is recorded as a regression floor, and the existing production paths still pass unchanged.
