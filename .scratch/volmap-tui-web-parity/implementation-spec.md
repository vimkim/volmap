# Volmap TUI terminal-parity implementation specification

Status: superseded

Superseded by: [Volmap focused TUI implementation specification](../volmap-tui-focused-inspector/implementation-spec.md). The completed Wayfinder map and its resolved decision tickets remain historical design evidence.

Minimum semantic baseline: `cba72cd` (`feat: attribute pages and sectors to their table in web, TUI, and JSON`)

Integration baseline: the implementation branch's merge-base; every behavior passing there remains a regression gate.

## How to use this specification

This file is the implementation index and delivery plan. The linked Wayfinder tickets are normative: they retain the exact contracts, evidence, edge cases, rejected alternatives, and verification lists. This file deliberately does not restate their tables or create a second source of truth.

When an implementation choice appears constrained here, follow the linked owning ticket. When a private detail is not constrained, choose it behind the accepted seam and test it without widening that seam. A product-level contradiction or missing decision must return to Wayfinder; it must not be settled in code or by changing a golden.

The accepted precedence is explicit: [Set volume viewport and rendering resource budgets](issues/07-set-viewport-resource-budgets.md) applies the later seven-column Page strip from [Define semantic color, glyph, and fallback mappings](issues/06-define-semantic-terminal-rendering.md), replacing the early prototype's narrow Sector-card geometry without changing the accepted Atlas hierarchy.

No product, interaction, shared-interface, state, rendering, resource, accessibility, compatibility, or verification decision remains unresolved at assembly time.

## Decision index

| Concern | Normative decision |
| --- | --- |
| Product hierarchy and responsive interaction | [Prototype terminal interaction parity across Volume, Sector, and Page](issues/01-prototype-terminal-interaction-parity.md) |
| Shared typed facts, geometry, diagnostics, revisions, and enrichment seam | [Define the shared projection boundary for terminal parity](issues/02-define-shared-projection-boundary.md) |
| Navigation, focus, trail, filters, findings, overlays, scroll, and adoption state | [Define the TUI navigation, focus, and history state model](issues/03-define-navigation-focus-history-model.md) |
| Renderer, terminal host, composition/presentation transaction, and dependency boundary | [Choose the terminal rendering architecture and dependency boundary](issues/04-choose-rendering-architecture.md) |
| Automatic work, progress, cancellation, publication races, offers, and retry | [Define automatic enrichment and immutable-revision transitions](issues/05-define-enrichment-revision-lifecycle.md) |
| Semantic channels, profiles, legends, text safety, width, and contrast | [Define semantic color, glyph, and fallback mappings](issues/06-define-semantic-terminal-rendering.md) |
| Complete-volume windows, reservoir, virtualization, redraw, memory, and latency | [Set volume viewport and rendering resource budgets](issues/07-set-viewport-resource-budgets.md) |
| Fixtures, stable assertions, gate families, browser/PTY evidence, release, and cutover | [Define the TUI parity verification contract](issues/08-define-parity-verification-contract.md) |

## Scope and non-goals

Implement one production Atlas TUI with terminal-native parity for the existing web viewer's Volume → Sector → Page hierarchy. It consumes the same normalized inspection facts, preserves exact immutable revision identity, shows real occupancy and exhaustive safe Page distribution/Slot structure, retains Page/Sector file-class-table attribution, and keeps the existing TUI utility features and accelerators named by the state-model decision.

This work does not redesign web, CLI, JSON/JSONL, deterministic HTML, storage decoding, disclosure boundaries, or Inspection resource semantics. It does not add browser-like history or browser-only top-level Slot/OOS routes, require pixel matching, expose payload/raw/ciphertext/key data, or provide a full-screen interface below 60×20. The complete exclusions remain in the [parent Wayfinder map](map.md).

## Accepted architecture

```text
Inspection graph and validated format facts
                 |
                 v
       Projection workspace
   exact immutable typed frames + enrichment
          |                     |
          |                     +---- existing web adapter (unchanged behavior)
          v
       AtlasMachine <---- typed input, layout commits, worker signals
          |
     AtlasStep: immutable AtlasScene + ordered AtlasEffects
          |
          v
       AtlasRenderer ---- compose ----> PreparedFrame
          |                                  |
          |                       successful present/flush
          |                                  |
          +<--------- generation-stamped LayoutCommit
                                             |
                                      Crossterm terminal host
```

The modules form deep seams, not a public widget or job framework:

- The Projection workspace is the only shared semantic seam. It owns exact-revision retention, typed projection and presentation-neutral derivation, bounded contiguous queries, cooperative enrichment arbitration, immutable publication, diagnostics, coverage, and invalidation. It never exposes raw bytes, validated decoder objects, HTTP types, or terminal presentation.
- `AtlasMachine::start` and `AtlasMachine::advance` own deterministic adapter state and are the only navigation/state interface. They consume semantic events and emit one immutable scene plus ordered effects. They do not depend on Crossterm, ANSI, rectangles, channels, HTTP, or browser history.
- `AtlasRenderer::compose` and opaque `PreparedFrame::present` own Atlas-specific cell composition, adaptive placement, sanitized display text, focus/scroll/hit geometry, diffing, and the post-flush layout commit. They do not query inspection state, interpret identity from labels, or mutate navigation.
- The private terminal host owns TTY validation, capability profile, raw/alternate-screen/mouse/cursor lifecycle, normalized events, one bounded enrichment worker, redraw scheduling, presentation, and cleanup. Production uses Crossterm; deterministic tests use the accepted scripted/recording adapters.
- Keep the CLI-facing `tui::run` entry point. The production cutover replaces the legacy state/draw path; it does not translate or layer it beneath Atlas.

Use the precise interfaces and ownership matrices in the [Projection workspace](issues/02-define-shared-projection-boundary.md), [AtlasMachine](issues/03-define-navigation-focus-history-model.md), [AtlasRenderer](issues/04-choose-rendering-architecture.md), and [enrichment lifecycle](issues/05-define-enrichment-revision-lifecycle.md) decisions.

## Cross-module invariants

These invariants apply to every delivery slice:

- One scene, cache entry, frame, status, and layout commit refers to one exact snapshot and immutable revision. No `latest` substitution, partial adoption, or cross-revision join is allowed.
- Volume topology is complete and ordered; projection and rendering may window it but never sample it. Every Sector contains exactly 64 ordered physical Pages.
- One atomic Page projection carries its Page facts, detail disposition, complete safe Slot directory, and exhaustive byte map. Geometry comes only from a validated slotted structure and conserves all 16,344 content bytes.
- Availability, allocation, physical type, occupancy-known/unknown/not-applicable, detail support, TDE state, attribution, diagnostics, coverage, outcome, focus, and selection remain independent typed dimensions.
- The typed Atlas trail is the only breadcrumb/back-stack source. Focus and content anchors use stable semantic identity, never coordinates, raw rows, vector positions, messages, subject strings, or rendered labels.
- Input and pointer paths converge on the same semantic actions. Only geometry from a successfully flushed, generation-matching layout commit may resolve pointer input.
- Candidate enrichment facts remain invisible until exact immutable publication and an authorized whole-trail adoption. User input wins a simultaneous completion race; cancellation can revoke adoption authority but cannot erase an already published revision.
- Every source-derived string passes through the one `TerminalText` path before measurement or placement. Style is separate metadata; unsafe controls cannot reach terminal output.
- Presentation limits affect cache/recomputation only. They never change Inspection coverage, outcome, diagnostics, revision, or reachability.
- Shared refactoring preserves all merge-base web, CLI, JSON/JSONL, deterministic HTML, disclosure, dependency, and release behavior.

## View, interaction, and adaptive layout contract

Atlas uses replacement Volume, Sector, and Page screens over one displayed exact revision. Volume is a complete scrollable Sector-card mosaic; Sector is one exhaustive 8×8 Page grid; Page exposes Facts, Distribution, Slots, Chain, Findings, and Coverage. Descent, ascent, breadcrumbs, selectors, findings, filters, accelerators, modal precedence, focus restoration, and independent semantic anchors follow the [state-model decision](issues/03-define-navigation-focus-history-model.md).

Layout tiers follow the [rendering-architecture decision](issues/04-choose-rendering-architecture.md): wide at 120×36, stacked at 80×24, compact at 60×20, and reversible suspension below that. Wide Page Facts and Distribution are simultaneous; stacked preserves all regions in one workspace; compact uses explicit regions/tabs. The definitive Sector-card extent, packing, canvas, row window, and scroll behavior are owned by the [viewport-budget decision](issues/07-set-viewport-resource-budgets.md), not the illustrative dimensions of the original prototype.

The title, breadcrumb, focused descriptor, status, contextual legends, prompts, progress, notices, and overlays must preserve the semantic content and precedence established by the [accepted prototype](issues/01-prototype-terminal-interaction-parity.md), [state model](issues/03-define-navigation-focus-history-model.md), and [semantic-rendering decision](issues/06-define-semantic-terminal-rendering.md). Mouse adds no action unavailable to keyboard-only operation.

## Enrichment and immutable revision contract

The Projection workspace supplies an exhaustive typed Page enrichment disposition and rechecks eligibility, exact head, source stability, cancellation semantics, and admission. Atlas issues at most one automatic attempt per Page visit/exact base/target, keeps the old revision visible, displays only trusted progress, and retains one physical worker with bounded newest intent/progress state.

The exact cancellation, final-publication precedence, diagnostic/partial publication behavior, automatic-adoption match, whole-trail transaction, one exact revision offer, explicit retry, snapshot invalidation overlay, and web compatibility rules are normative in [Define automatic enrichment and immutable-revision transitions](issues/05-define-enrichment-revision-lifecycle.md). Do not infer eligibility or outcome from allocation, messages, HTTP behavior, or a browser predicate.

## Semantic and accessible presentation contract

The renderer uses the closed seven-channel Page strip and full semantic vocabulary from [Define semantic color, glyph, and fallback mappings](issues/06-define-semantic-terminal-rendering.md). The four supported profiles are ANSI/Unicode, monochrome/Unicode, ANSI/ASCII, and monochrome/ASCII. Profile or tier may compact presentation but cannot remove facts, controls, topology, focus/scroll identity, legends, known-versus-unknown distinctions, exact Distribution rows, or Slot entries.

ANSI and glyphs reinforce rather than carry meaning alone. Focused descriptors and contextual Help expand every compact token. The Page raster is a navigation aid; exact ordered byte-range and Slot rows remain authoritative and reachable. Controlled contrast evidence, monochrome structural equivalence, hostile source-label sanitization, grapheme-safe clipping, deterministic normal-width measurement, continuation-cell correctness, and the proposed exact Unicode dependencies are all gated by that decision and the [rendering architecture](issues/04-choose-rendering-architecture.md). Ratatui is not part of this design.

## Viewport, scheduling, and resource contract

Use the private exact-revision reservoir, complete-card row window, semantic anchors, Page-row virtualization, invalidation matrix, one-dirty-frame scheduler, input-first ordering, redraw cadence, and numeric ceilings from [Set volume viewport and rendering resource budgets](issues/07-set-viewport-resource-budgets.md). The Terminal rendering budget is separate from Inspection's operational `ResourcePolicy`.

The hard contract includes bounded contiguous Projection requests, constant resident work across Volume size, the capped logical canvas, complete Sector cards, bounded prepared Page rows, the 16 MiB incremental Atlas heap ledger, zero idle redraw, and controlled cold/warm/input latency gates. Private representation may vary only while it continues to satisfy those public cardinality, identity, completeness, memory, and latency assertions.

## Verification and cutover contract

[Define the TUI parity verification contract](issues/08-define-parity-verification-contract.md) is the acceptance authority. Implementation must create its checked-in parity matrix and canonical corpus first, then accumulate evidence through all eight gate families: `PAR-PROJECTION`, `PAR-STATE`, `PAR-RENDER`, `PAR-HOST`, `PAR-WEB`, `PAR-DISCLOSURE`, `PAR-RESOURCE`, and `PAR-RELEASE`.

Stable assertions stop at typed facts, state/effects, normalized semantic cells, committed geometry, reachability, and lifecycle behavior. Raw ANSI chunking, screenshots, browser pixels, incidental DOM/CSS structure, allocator RSS, and uncontrolled timing are review aids only. The 36 core CellGrid/LayoutCommit artifacts, focused goldens, deterministic traces and properties, scripted host, real PTY, locked browser, disclosure sentinels, memory ledger, performance evidence, and one-candidate release proof remain exactly as specified in the verification decision.

Parity is complete only when the same exact candidate commit passes every ordinary and controlled blocking gate, production `tui::run` exclusively uses the accepted modules, and the legacy TUI state/drawing/hit-testing/truncation/too-small behavior and obsolete tests are deleted. Missing infrastructure, retries, quarantines, weakened assertions, raised budgets, removed profiles/tiers, or unreviewed golden replacement cannot create a pass.

## Dependency-ordered delivery plan

Each work package below is a suitable `/to-tickets` unit. Split further only along the same seams and keep each package's completion criterion on the public interface. Tests and fixtures named by a package are written before or with its production change.

```text
D0 -> {D1, D3}
D1 -> D2
{D2, D3} -> D4
{D1, D2} -> D5
{D3, D4, D5} -> D6
{D4, D5, D6} -> D7
{D0 ... D7} -> D8
```

### D0 — Freeze the evidence vocabulary

Depends on: none.

Create the checked-in parity matrix, canonical exact-revision corpus/builders, stable gate names, human-reviewable golden format, deterministic trace runner/reducer, and isolated test-tool locks required by the verification decision. Capture the implementation merge-base suite without changing expected behavior.

Completion: every accepted requirement from the first seven decisions maps to a fixture, interface, invariant, and blocking job; the corpus covers the named semantic, revision, hostile-text, large-Volume, maximum-Page, and disclosure cases without inventing adapter-specific storage facts.

### D1 — Establish the Projection workspace

Depends on: D0.

Move presentation-neutral Page distribution/Slot derivation, typed diagnostics and affected-entity references, exact immutable revision retention, bounded Volume/Sector/Page queries, Page disposition, enrichment publication arbitration, and invalidation overlay behind the accepted Projection workspace interface. Adapt web internally where required without changing its observable behavior.

Completion: `PAR-PROJECTION`, the handler-level portion of `PAR-WEB`, and shared disclosure tests pass; old revisions remain immutable; complete geometry and 64-Page/contiguous-window contracts hold; all merge-base web/CLI/export schemas and behavior remain unchanged.

### D2 — Implement the deterministic AtlasMachine

Depends on: D1.

Implement the closed state, typed Atlas trail, semantic anchors, selectors/filters/findings, overlays, accelerators, keyboard/mouse semantic actions, resize generation, immutable scenes, and ordered effects against the Projection workspace. Do not integrate terminal I/O.

Completion: all non-worker `PAR-STATE` scenarios and seeded properties pass through `AtlasMachine::start`/`advance`, including hierarchy restoration, filters, typed findings, every retained accelerator, 59×19 suspension/recovery, and stale-layout rejection.

### D3 — Implement semantic cells and AtlasRenderer

Depends on: D0. It may proceed in parallel with D1 and D2 against canonical semantic scenes.

Add the private `TerminalText`, bounded cell frame, semantic encoder, tier composers, placement/geometry recorder, opaque prepared frame, recording/fault presenter, and post-flush commit transaction. Add the exact Unicode dependencies only after their implementation-time supply-chain audit; do not add Ratatui.

Completion: `PAR-RENDER` passes the 36 core and focused goldens, four profiles, hostile text, complete Page/Slot geometry, hit/focus/scroll invariants, deterministic composition, continuation-cell handling, and compose/write/flush fault behavior.

### D4 — Integrate Atlas screens and committed geometry

Depends on: D2 and D3.

Connect immutable Atlas scenes to the renderer and feed only successfully presented layout commits back into AtlasMachine. Implement all Volume, Sector, Page, utility-region, prompt/help/progress/notice, and reversible-too-small scenes without yet making Atlas the production `tui::run` path.

Completion: paired keyboard/mouse semantic traces and all tier/profile/layout transitions pass through the real machine-renderer transaction; no input code reconstructs coordinates and no frame mixes revisions.

### D5 — Integrate the enrichment lifecycle

Depends on: D1 and D2. It may proceed in parallel with D4 until rendered progress/notice integration.

Implement the private Page-visit/request protocol, one-worker admission, capacity-one progress, draining and replaceable intent, cancellation, exact completion matching, whole-trail adoption, revision offers, explicit retry, invalidation overlay, and quit/fault cleanup using the Projection workspace.

Completion: the enrichment portions of `PAR-PROJECTION` and `PAR-STATE` pass every eligibility, progress, publication, cancellation, stale/late signal, offer, rollback, invalidation, and input-first race; existing web enrichment responses remain unchanged.

### D6 — Enforce viewport and terminal resource budgets

Depends on: D3, D4, and D5.

Add the private 64-Sector reservoir, complete-card row window and overscan, atomic refills, Page-row virtualization, invalidation rules, one-dirty-frame scheduling, resize/progress coalescing, memory ledger, work counters, and recording-presenter benchmarks.

Completion: deterministic `PAR-RESOURCE` cardinality/redraw/ledger tests pass for the lazy maximum topology, exhaustive 257-Sector traversal, real sparse fixture, maximum Page rows, every invalidation, adversarial extents, and 100,000 progress events; controlled latency evidence meets the accepted thresholds.

### D7 — Integrate the production terminal host

Depends on: D4, D5, and D6.

Implement the Crossterm host, capability/profile resolution, TTY validation, partial-entry rollback, raw/alternate-screen/mouse/cursor lifecycle, normalized event loop, bounded worker ownership, frame presentation, typed terminal faults, and best-effort scoped panic cleanup. Route a non-production/test Atlas construction through the preserved CLI entry while legacy remains the production fallback until release gates pass.

Completion: scripted `PAR-HOST` and real static-binary Expect PTY tests pass for input/mouse/resize delivery, normal and fault exit, cancellation during exit, partial entry, write/flush failure, and restoration; the host adds no semantic or projection policy.

### D8 — Prove compatibility, cut over, and delete legacy

Depends on: D7 and completion of every prior package's blocking evidence.

Run the complete Rust, browser, PTY, disclosure, resource/performance, dependency/license/SBOM, reproducibility, static-musl, and distribution evidence on one exact candidate. Fix failures through their owning seam without rebaselining accepted behavior. Only after all gates pass, switch production `tui::run` atomically to Atlas and delete the legacy implementation and obsolete tests in the same candidate.

Completion: every `PAR-*` family and controlled blocking job names and passes the same candidate; all merge-base adapter/release behavior passes; no production legacy renderer, translation layer, second state model, or forbidden dependency remains.

## Handoff audit

- All eight prerequisite Wayfinder decisions are resolved and linked above.
- No unresolved product decision or new implementation-blocking ticket was found.
- No new domain term is introduced; use the glossary in [`CONTEXT.md`](../../CONTEXT.md).
- Prototype branches are design evidence only and must not be merged into production.
- Private file layout, helper names, and cache representation remain implementation choices only when they stay behind the accepted interfaces and satisfy all gates.
- This Wayfinder effort ends at specification assembly. Production implementation begins only after this plan is converted into scoped delivery tickets.
