# 04: Present the first Atlas Volume screen

**What to build:** Deliver the first complete non-production Atlas path: exact-revision Volume facts flow through `AtlasMachine`, an immutable semantic scene, `AtlasRenderer`, and the scripted presenter into a reviewable Volume screen. Follow the accepted [state model](../../volmap-tui-web-parity/issues/03-define-navigation-focus-history-model.md), [rendering architecture](../../volmap-tui-web-parity/issues/04-choose-rendering-architecture.md), and [semantic rendering contract](../../volmap-tui-web-parity/issues/06-define-semantic-terminal-rendering.md).

**Blocked by:** 02: Expand the exact-revision Projection workspace.

**Status:** superseded

**Superseded by:** [Volmap focused TUI implementation specification](../../volmap-tui-focused-inspector/implementation-spec.md).

- [ ] `AtlasMachine::start`/`advance` produce deterministic Volume state, one immutable scene, and ordered effects without terminal, HTTP, or worker types in the interface.
- [ ] `AtlasRenderer::compose` and opaque presentation produce one bounded normalized cell frame and release a generation-stamped `LayoutCommit` only after a successful complete flush.
- [ ] The Volume screen renders at 120×36, 80×24, and 60×20 plus reversible 59×19 suspension under all four terminal presentation profiles.
- [ ] Every visible Page preview preserves the fixed allocation, physical-type, occupancy, finding, focus, and selection channels; focused descriptions expose exact typed values and attribution.
- [ ] Title, breadcrumb, status, contextual legend, focus controls, and semantic hit/focus/scroll geometry are produced from one exact revision without coordinate reconstruction by input code.
- [ ] One `TerminalText` path safely handles controls, bidi formatters, empty labels, grapheme clipping, display width, continuation cells, ASCII fallback, and hostile file/class/table attribution.
- [ ] Compose, short-write, and flush fault tests prove that invalid geometry, frame-cache advancement, and layout commits are never published.
- [ ] The Volume subset of `PAR-STATE`, `PAR-RENDER`, and the core golden matrix passes while production `tui::run` remains on the legacy path.
