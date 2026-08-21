Label: wayfinder:map
Status: open

# Chart terminal interaction parity for the Volmap TUI

## Destination

An implementation-ready specification and decision index for redesigning the Volmap TUI with terminal interaction parity across the existing web viewer's Volume → Sector → Page hierarchy. The map is complete when no shared-projection, session-state, navigation, layout, rendering, resource, accessibility, or verification decision remains implicit before implementation begins.

## Notes

- This map plans the redesign; it does not implement the production TUI.
- Source baseline: this repository at commit `cba72cd` (`feat: attribute pages and sectors to their table in web, TUI, and JSON`).
- The implemented web viewer and [`docs/images`](../../docs/images) are the behavioral and visual reference. The TUI remains a projection of the same normalized inspection graph and must not parse volume bytes or invent terminal-only storage facts.
- `Terminal interaction parity` has the meaning recorded in [`CONTEXT.md`](../../CONTEXT.md): preserve the web viewer's Volume → Sector → Page drill-down and semantic distinctions through terminal-native layout, rendering, and controls rather than pixel matching.
- The accepted scope covers those three views with real page occupancy and real slot/record distribution. Browser-only Slot/OOS routes are not parity requirements, while existing TUI Chain, Findings, Coverage, filters, search, mouse, and keyboard accelerators remain in scope.
- Page and Sector file/class/table attribution introduced by `cba72cd` is shared semantic data and a compatibility requirement; web and TUI adapters may format it differently but must consume the same typed facts.
- Shared production code may change where needed to extract web-private distribution logic and provide revision-aware TUI state, but the redesign must not change web behavior.
- Target layouts are optimized for 120×36 and larger, provide full stacked parity at 80×24, and retain a functional compact fallback down to the current 60×20 minimum.
- Navigation uses replacement screens: `Enter` descends, `Esc` or `Backspace` ascends, and existing sector/volume/search/finding accelerators remain available. Wide page detail uses facts plus distribution panes; narrow detail stacks or tabs them.
- Opening a supported page automatically requests bounded deep enrichment, visibly handles progress and cancellation, and adopts only the explicitly returned immutable revision.
- ANSI color and Unicode block glyphs are the primary presentation; semantic labels and monochrome/ASCII fallback are mandatory.
- Every human decision session should consult the `grilling` and `domain-modeling` skills. Interface and module-boundary decisions should also consult `codebase-design`; visual decisions should use `prototype`.
- This repository uses the local Markdown tracker under `.scratch`; the files under [`issues/`](issues) are the map's child-issue query. A child is claimed by recording its assignee before work starts.
- Durable work-tracker item: `12`.

## Decisions so far

- [Prototype terminal interaction parity across Volume, Sector, and Page](issues/01-prototype-terminal-interaction-parity.md) — Atlas is accepted as the replacement-screen hierarchy, with simultaneous wide Page panes, stacked 80×24 parity, tabbed 60×20 fallback, and equivalent semantic/input fallbacks.
- [Define the shared projection boundary for terminal parity](issues/02-define-shared-projection-boundary.md) — A shared Projection workspace owns exact-revision semantic projections and cooperative enrichment, while web and TUI retain navigation, scheduling, transport, and rendering state.
- [Define the TUI navigation, focus, and history state model](issues/03-define-navigation-focus-history-model.md) — A deterministic AtlasMachine reducer owns the structural Atlas trail, focus restoration, modal precedence, semantic scrolling, input equivalence, and atomic exact-revision adoption.
- [Choose the terminal rendering architecture and dependency boundary](issues/04-choose-rendering-architecture.md) — A repository-owned AtlasRenderer over pinned Crossterm atomically composes cell frames and semantic layout commits, with private Atlas-specific primitives, post-flush activation, and replace-don't-layer migration.
- [Define automatic enrichment and immutable-revision transitions](issues/05-define-enrichment-revision-lifecycle.md) — Atlas privately manages one bounded exact-base attempt, trusted progress, deterministic cancellation, atomic whole-trail adoption, and explicit late revision offers while the Projection workspace owns eligibility and immutable publication.
- [Define semantic color, glyph, and fallback mappings](issues/06-define-semantic-terminal-rendering.md) — A closed Atlas semantic encoder preserves allocation, type, occupancy, findings, focus, selection, Page geometry, and Slot state across four equivalent ANSI/monochrome and Unicode/ASCII profiles with safe display-column text.
- [Set volume viewport and rendering resource budgets](issues/07-set-viewport-resource-budgets.md) — Atlas uses a private 64-Sector exact-revision reservoir, fixed complete-card row windows with one-row overscan, semantic focus and row virtualization, deterministic invalidation/redraw, and a separate 16 MiB terminal-rendering budget with measurable work and latency gates.
- [Define the TUI parity verification contract](issues/08-define-parity-verification-contract.md) — Acceptance uses interface-aligned Projection, Atlas state, semantic cell/geometry, scripted-host, real PTY/browser, non-disclosure, resource/performance, and release evidence from one exact candidate commit before the legacy TUI is removed.

## Out of scope

- Implementing or shipping the production TUI within this Wayfinder effort.
- Redesigning the web viewer, CLI, JSON/JSONL, or deterministic HTML output; shared refactoring must preserve their observable behavior.
- Pixel-for-pixel imitation of browser CSS or reproduction of browser-only URLs, Back/Forward mechanics, hover behavior, and responsive breakpoints.
- Requiring dedicated top-level Slot or OOS screens beyond the requested Volume, Sector, and Page parity; their normalized facts and existing terminal Chain affordance remain available where relevant.
- Changing CUBRID parsing, on-disk format interpretation, inspection facts, disclosure boundaries, or resource-policy semantics.
- Supporting terminals smaller than 60 columns by 20 rows as a full-screen interactive interface.
