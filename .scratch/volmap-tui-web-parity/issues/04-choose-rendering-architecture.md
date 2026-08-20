Label: wayfinder:grilling
Type: grilling
Status: resolved
Assignee: codex
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Prototype terminal interaction parity across Volume, Sector, and Page](01-prototype-terminal-interaction-parity.md), [Define the shared projection boundary for terminal parity](02-define-shared-projection-boundary.md)

# Choose the terminal rendering architecture and dependency boundary

## Question

Should production retain and deepen the current manual `crossterm` renderer, introduce a terminal widget/layout library such as Ratatui, or place a small repository-owned widget layer over `crossterm`? Decide using the accepted prototype, adaptive-layout complexity, focus and hit-testing needs, display-column correctness, static-musl and reproducible-release constraints, dependency/license/SBOM cost, testability, and the requirement to keep a deep terminal interface rather than scatter view logic across key handling and drawing code.

## Answer

The user accepted every recommendation over two decision rounds and then confirmed the complete shared understanding. Production should retain the existing pinned `crossterm` terminal dependency and replace the current manual drawing path with one repository-owned, Atlas-specific deep rendering module. Do not add Ratatui, do not relax the existing dependency policy to admit it, and do not publish a general widget toolkit.

The chosen shape is deliberately between the rejected extremes. Continuing to call `queue!` directly from screen code would preserve the shallow coupling between projection reads, fixed layout coordinates, mouse hit testing, styling, clipping, and output. A general repository widget framework would recreate a terminal UI library that Volmap does not otherwise need. The accepted renderer owns only the reusable machinery required to present the fixed Atlas Volume → Sector → Page experience and hides that machinery behind a small interface.

### Evidence and dependency decision

At the source baseline, [`src/tui.rs`](../../../src/tui.rs) is a 912-line module in which the event loop calls drawing directly, terminal lifecycle is tied to concrete `Stdout`, mouse handling independently reconstructs fixed row and column geometry, every draw clears the screen, and `truncate` counts Unicode scalar values instead of display columns. Its four private helper tests do not exercise complete frames, terminal lifecycle, responsive tiers, or the relationship between painted controls and hit regions. Retaining `crossterm` is not an endorsement of that structure; the direct renderer is to be replaced.

Ratatui 0.30.2 is individually compatible with the repository's Rust version, MIT license allowlist, and pinned Crossterm 0.29 generation. It provides useful cell buffers, diffing, layouts, widgets, and a test backend. However, a minimal `crossterm_0_29` normal-dependency probe adds 33 crate names relative to the current musl graph and necessarily resolves duplicate `hashbrown` 0.16/0.17 and Syn 2/3 package generations. That cannot pass the repository's categorical `multiple-versions = "deny"` release policy. Its Crossterm adapter also enables Crossterm's default features instead of preserving Volmap's current `default-features = false, features = ["events"]` selection. Ratatui 0.29 instead requires Crossterm 0.28 and conflicts with the current exact pin.

Changing those supply-chain constraints would be a materially broader product decision for machinery that still would not own Atlas focus, semantic scroll anchors, typed hit regions, revision identity, or source-label sanitization. Ratatui is therefore rejected at this baseline. A future reconsideration requires the exact resolved graph—not only the top-level license—to pass the then-current locked license, duplicate-version, static-musl, reproducibility, notice, and SBOM gates.

### Deep renderer interface

Introduce one `AtlasRenderer` module inside the TUI inspection adapter. Its external seam consumes only the immutable semantic scene accepted in [Define the TUI navigation, focus, and history state model](03-define-navigation-focus-history-model.md) and repository-owned terminal presentation inputs:

```rust
pub struct AtlasRenderer { /* private layout policy and caches */ }

impl AtlasRenderer {
    pub fn compose(
        &mut self,
        scene: &AtlasScene,
        surface: RenderSurface,
    ) -> Result<PreparedFrame, RenderFault>;
}

pub struct RenderSurface {
    pub extent: CellExtent,
    pub generation: LayoutGeneration,
    pub profile: TerminalProfile,
}

impl PreparedFrame {
    pub fn present(
        self,
        presenter: &mut TerminalPresenter,
    ) -> Result<LayoutCommit, PresentError>;
}
```

`PreparedFrame` is opaque. It owns one complete repository cell frame and the matching generation-stamped `LayoutCommit`. The commit carries the resolved terminal extent and tier, compact directional focus topology, semantic scroll extents, and precedence-ordered `ControlId`/`ScrollRegion` hit regions. No Ratatui type, Crossterm command, ANSI sequence, raw terminal rectangle, widget callback, terminal writer, Projection workspace handle, or inspection parser crosses this interface.

`AtlasScene` is the sole content source. Rendering never checks out an Inspection revision, queries the Projection workspace, changes the Atlas trail, schedules enrichment, parses a selector or diagnostic message, or derives identity from a rendered label. File/class/table attribution is ordinary typed scene content; it is sanitized and presented but never becomes navigation identity.

The interface is the renderer test surface. Private helpers may be split internally, but no public widget registry or alternate rendering port is justified. Deleting the module would scatter responsive tier selection, display-column text, cell clipping, style precedence, focus geometry, hit testing, scroll extents, and terminal encoding back across screen and input callers, so the module earns its depth.

### Atomic composition and presentation

One render transaction has this order:

1. The event loop invalidates the installed layout generation after resize or another presentation-changing transition.
2. `AtlasMachine::advance` returns one immutable `AtlasScene` and ordered effects.
3. `AtlasRenderer::compose` sanitizes text, selects the largest supported tier, lays out and paints a complete off-screen cell frame, and records focus, hit, and scroll geometry from those exact placements.
4. Composition validates cell bounds, focus targets, scroll extents, clipped hit regions, and equal-precedence non-overlap before returning an opaque `PreparedFrame`.
5. `PreparedFrame::present` diffs against the last successfully presented frame, writes all changes, and flushes through the terminal presenter.
6. Only successful presentation updates the presenter's prior-frame cache and releases the matching `LayoutCommit` to `AtlasMachine` through `AtlasEvent::LayoutCommitted`.
7. Only after that commit is installed may a pointer coordinate resolve against the new generation.

If composition fails, no terminal bytes or geometry are published. If write or flush fails, no `LayoutCommit` is returned, the previous-frame cache is not advanced, and the interactive session terminates through typed terminal I/O failure because the physical screen may be partially updated. A stale-generation pointer remains the already-accepted no-op interaction outcome.

The same placement operation paints a control and records its semantic geometry. Input handling must never reconstruct rows, columns, tab spans, or card widths separately. Overlay regions shadow lower-precedence screen regions; equal-precedence regions are clipped and non-overlapping. Mouse and keyboard actions continue to converge on the same semantic Atlas action.

### Private implementation shape

Keep the following concepts private to the renderer implementation:

- A bounded `CellGrid` stores sanitized symbols and semantic styles separately from terminal escape encoding.
- `TerminalText` performs control sanitization, grapheme-safe clipping, display-column measurement, wrapping, padding, and ellipsis.
- A tier planner implements the accepted wide, stacked, compact, and reversible too-small layouts.
- A geometry recorder emits focus topology, semantic scroll extents, and hit regions from actual placements.
- Typed Volume, Sector, Page, Page-region, status, breadcrumb, prompt, help, progress, and overlay composers render the closed Atlas scene vocabulary.
- A stateful presenter performs frame diffing and Crossterm encoding only after a valid complete frame exists.

These are not public widgets. Layout uses explicit checked formulas and small private helpers for the three accepted screen families rather than a public constraint solver or stringly render tree. A future Atlas region extends the closed internal scene and its typed presentations; callers do not register callbacks or learn a widget protocol.

Tier selection is deterministic and uses both terminal dimensions:

- `width >= 120 && height >= 36`: wide Atlas layout.
- Otherwise `width >= 80 && height >= 24`: stacked standard layout.
- Otherwise `width >= 60 && height >= 20`: compact tabbed layout.
- Otherwise: a reversible too-small frame that retains state and accepts resize, quit, and cancellation.

Wide Page Facts and Distribution remain simultaneous independent scroll regions with Slots continuing below. Standard maps the same semantic regions into one stacked workspace. Compact maps them into the accepted Page-region tabs. Sector remains the exhaustive physical 8×8 Page grid. Volume remains complete and unsampled while painting only the viewport window; compact analytical grid topology permits directional focus without allocating adjacency state proportional to off-screen sectors.

Composition must be deterministic for the same `AtlasScene`, `RenderSurface`, and implementation version. Locale, clock, randomness, terminal queries, and prior physical output cannot change the semantic frame or `LayoutCommit`; only private performance caches may vary without affecting results.

### Text and presentation dependency seam

The renderer may take direct, exact, independently audited dependencies on `unicode-width` and `unicode-segmentation`. Do not rely on a transitive dependency to define Volmap's display behavior, and do not implement Unicode display width by counting bytes, scalar values, or ad hoc ranges. The exact sanitization, ambiguity policy, glyph vocabulary, color precedence, monochrome behavior, and ASCII encoding remain the decision owned by [Define semantic color, glyph, and fallback mappings](06-define-semantic-terminal-rendering.md).

Whatever that ticket selects must pass through one `TerminalText` path before measurement or cell placement. Source-derived labels, including class and table names, can be visibly replaced, clipped, or ellipsized but cannot inject C0/C1 controls, ESC, DEL, embedded line breaks, bidirectional controls, or terminal commands. Style remains separate metadata, so source text cannot manufacture ANSI. The ANSI/monochrome and Unicode/ASCII axes use the same semantic controls, focus topology, scroll regions, and content; a fallback changes presentation, never available facts or actions.

Every new dependency remains subject to exact pinning, `cargo deny`, deterministic notice and CycloneDX SBOM regeneration, the locked static `x86_64-unknown-linux-musl` build, and the two-checkout reproducibility audit. This decision authorizes only the narrow display-text dependencies, not a general exception to the dependency policy.

### Terminal host and error ownership

Keep one adjacent repository-owned terminal host over Crossterm. It owns TTY validation, capability/profile selection, raw-mode and alternate-screen entry, cursor visibility, mouse capture, input normalization, resize delivery, terminal presentation, and teardown. Production uses the Crossterm-backed presenter; deterministic renderer tests use a recording or fault-injecting presenter. These are two real adapters at the private presentation seam.

The existing RAII cleanup principle remains for normal and typed-error exits, but it must cover every partially completed entry step. Because release builds use `panic = "abort"`, `Drop` cannot by itself restore a terminal after a panic. The host installs a scoped best-effort panic hook before terminal entry, delegates to the previous hook after attempting cursor/mouse/alternate-screen/raw-mode restoration, and restores the prior hook on ordinary exit. A subprocess test must exercise this path; the hook is a last-resort cleanup attempt, not a promise that arbitrary process termination is recoverable.

`RenderFault` is limited to implementation or bounded-allocation defects such as impossible extents, arithmetic overflow, invalid scene invariants, out-of-range geometry, invalid focus topology, overlapping equal-precedence hit regions, or post-sanitization text invariants. Unsupported, unavailable, encrypted-opaque, partial, unknown, filtered, narrow, and too-small states are renderable scene facts rather than renderer errors. `PresentError` owns terminal write and flush failure. Inspection, Projection workspace, enrichment, selector, and navigation outcomes remain outside the renderer.

Composition and presentation work is `O(viewport cells + visible semantic items)` with memory bounded by terminal cell area plus emitted visible geometry and the prior committed frame. It never scales with volume bytes or materializes off-screen sector cards merely to paint the viewport. Precise maximum terminal area, redraw-latency, memory, overscan, and binary-size acceptance thresholds belong to [Set volume viewport and rendering resource budgets](07-set-viewport-resource-budgets.md).

### Migration and deletion plan

Do not incrementally wrap or translate the legacy `State`, `draw`, fixed-coordinate mouse code, or `detail_lines`. Introduce the new pure rendering path beside it and replace the old path after parity:

1. Add representative immutable `AtlasScene` fixtures and the repository-owned cell, surface, frame, commit, and presenter types.
2. Implement private text, geometry, tier, and typed screen composers through `AtlasRenderer::compose`; establish deterministic frame and topology gates before terminal I/O integration.
3. Implement the accepted `AtlasMachine` and connect its scenes and committed layouts to the new renderer while preserving the CLI-facing `tui::run` entry point.
4. Add the Crossterm terminal host and enrichment-event scheduling around the renderer and machine; the host installs a layout only after a successful presentation.
5. Run the complete semantic, PTY, dependency, release, and web-compatibility gates.
6. Cut `tui::run` over atomically, then delete the legacy `State`, direct `draw`, fixed coordinate constants and mouse reconstruction, scalar-count `truncate`, `TerminalTooSmall` exit, and obsolete private tests.

This is replace-don't-layer migration. During development the legacy and Atlas paths may coexist behind test-only or non-default construction, but production does not ship two renderers, translate legacy state into Atlas state, or retain old helper tests beneath the new interface tests.

### Compatibility gates

- Golden repository cell frames and matching `LayoutCommit` assertions for representative Volume, Sector, Page, prompt/help/progress, diagnostic, and too-small scenes at 120×36, 80×24, 60×20, and 59×19.
- The same scene matrix under ANSI/Unicode, monochrome/Unicode, ANSI/ASCII, and monochrome/ASCII profiles, proving semantic content, enabled controls, focus topology, hit regions, and scroll regions do not disappear with presentation capabilities.
- Property and fuzz coverage for arbitrary source labels, combining sequences, wide graphemes, zero-width input, embedded controls, clipping, padding, ellipsis, and wide-cell clearing. No output may overrun its surface, corrupt a neighbor cell, emit unsanitized control data, or alter semantic topology.
- Geometry validation proving every hit region comes from the painted control, all regions are clipped, equal-precedence regions are disjoint, focus edges name valid controls, scroll extents are bounded, and stale generations cannot activate.
- Paired keyboard and mouse Atlas traces at every tier, including hovered-pane scrolling, overlays, resize invalidation, and wide → compact → wide restoration.
- Injected short-write and flush failures proving no layout commit or previous-frame-cache advance occurs after failed presentation.
- PTY/subprocess tests for non-TTY rejection, partial and successful terminal entry, raw mode, alternate screen, mouse capture, cursor restoration, normal exit, every typed error exit, and best-effort panic-hook cleanup.
- A large-Volume renderer benchmark proving viewport-bounded painting without sampling or off-screen card materialization; numeric acceptance thresholds are supplied by ticket 07.
- The existing static-musl, exact dependency, `cargo deny`, notice, SBOM, deterministic two-checkout release, and static ELF gates after dependency changes.
- Existing Projection workspace, web route/history, Page distribution/JSON, immutable-revision, allocation/occupancy, and Page/Sector file/class/table attribution behavior remains unchanged.

No production implementation is made by this resolution. No separate ADR is needed because this claimed Wayfinder decision is already the durable rationale and implementation index. No new domain term is added: `AtlasRenderer`, `PreparedFrame`, and `LayoutCommit` are implementation vocabulary rather than inspection-domain language. No new ticket is created; the remaining semantic text details and numeric resource thresholds are already owned by tickets 06 and 07, and this decision removes the last migration-shape fog from the map.
