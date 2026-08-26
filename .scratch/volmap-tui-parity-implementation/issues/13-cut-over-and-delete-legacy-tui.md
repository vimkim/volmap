# 13: Cut over production and delete the legacy TUI

**What to build:** Make Atlas the only production TUI, remove the legacy implementation rather than layering or translating it, and prove terminal interaction parity on the exact final candidate commit.

**Blocked by:** 12: Complete blocking parity and resource evidence.

**Status:** superseded

**Superseded by:** [Volmap focused TUI implementation specification](../../volmap-tui-focused-inspector/implementation-spec.md).

- [ ] Production `tui::run` routes exclusively through the accepted Projection workspace, AtlasMachine, AtlasRenderer, and Crossterm terminal host.
- [ ] No production legacy-to-Atlas translation, second renderer, alternate state machine, web-derived semantic calculation, or generic widget/job framework remains.
- [ ] Legacy state, direct clear/draw path, fixed-coordinate mouse reconstruction, eager Page detail lines, scalar-count truncation, too-small exit behavior, and obsolete helper tests are deleted.
- [ ] The CLI entry and all merge-base web, CLI, JSON/JSONL, deterministic HTML, disclosure, schema, dependency, and release behavior remain unchanged except for the accepted Atlas TUI replacement.
- [ ] Every ordinary and controlled `PAR-*` job names and passes the same post-deletion candidate commit, including static-binary PTY/browser tests, resource/performance evidence, supply-chain audit, reproducibility, and distribution execution.
- [ ] The final tree contains the checked-in parity matrix, reviewed goldens, reproducible evidence metadata, and no production-only bypass or test-only acceptance path.
