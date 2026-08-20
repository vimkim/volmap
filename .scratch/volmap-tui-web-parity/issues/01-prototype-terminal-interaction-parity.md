Label: wayfinder:prototype
Type: prototype
Status: resolved
Assignee: codex
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: None

# Prototype terminal interaction parity across Volume, Sector, and Page

## Question

What concrete terminal interaction and visual hierarchy best translates the implemented web viewer's full-volume sector-card mosaic, focused 64-page sector, and page facts/distribution workspace across the accepted 120×36, 80×24, and 60×20 tiers? Build a cheap interactive prototype with representative real projections and at least two materially different responsive layouts; exercise keyboard and mouse descent/ascent, scrolling, resizing, occupancy-known and occupancy-unknown pages, findings, fragmented slotted pages, ANSI/Unicode rendering, and monochrome/ASCII fallback. Work with the user against the rendered artifact and link the accepted prototype from the resolution.

## Answer

### Accepted prototype

The user reviewed the browser-hosted terminal mock and explicitly accepted **A — Atlas exactly as shown**. The throwaway artifact is captured on branch `prototype/tui-web-parity` at commit `0df34908f91cc93c673b67f65a355db047ea5287`; open the [interactive prototype](/home/vimkim/temp/volmap-tui-web-parity-prototype/prototype/tui-web-parity.html) or its adjacent README. The branch is intentionally isolated from `main` and is a primary design source, not production code.

Atlas is the production design direction. **B — Ledger** and **C — Workbench** remain comparison evidence only: the production TUI does not replace the spatial hierarchy with ordered ledger rows and does not reserve persistent hierarchy or context rails. Downstream decisions may deepen Atlas's widgets and behavior, but must not silently switch to either rejected information hierarchy.

### View hierarchy

Atlas uses terminal-native replacement screens over one immutable inspection revision:

1. **Volume** is a complete, scrollable sector-card mosaic. Every sector card contains its ordered 8×8 preview of 64 pages; loading, caching, or viewport work may bound materialization but must never sample or omit sectors. Sector focus survives descent, ascent, and resize.
2. **Sector** is a focused 8×8 grid of all 64 physical pages in logical page order. Every cell keeps allocation class, physical type, occupancy-known versus occupancy-unknown, focus, and finding emphasis independently legible. `Enter` or the equivalent mouse action opens the focused Page; ascent restores the same cell.
3. **Page** presents normalized page facts and the exhaustive slotted-page distribution. At 120×36 and larger the facts and distribution are simultaneous panes, with the slot directory continuing below. At 80×24 the same content stacks in one scrollable workspace. At 60×20 Facts, Distribution, Slots, and Findings become explicit tabs so the compact tier remains functional without deleting a semantic region.

The title row retains the snapshot fingerprint, immutable revision, aggregate outcome, terminal tier, and rendering mode. The breadcrumb names the current Volume → Sector → Page path and provides the same ascent action as `Esc` or `Backspace`. A persistent status row reports focus, resize, enrichment, cancellation, and navigation outcomes without replacing canonical facts.

### Interaction contract demonstrated by the prototype

- Arrow keys provide roving focus within the active sector-card or page grid; `Enter` descends, while `Esc` or `Backspace` ascends and restores the prior focus.
- Mouse selection descends through the same hierarchy, and the wheel scrolls only the hovered workspace. Mouse operation adds no action unavailable from the keyboard.
- `[` and `]` move between sectors, `/` opens typed-selector navigation, `n` moves to the next finding, `Tab` cycles compact detail regions, and `?` opens contextual help. Existing accelerators not represented by a prototype control remain in the navigation ticket rather than being removed by this verdict.
- Resize recomputes layout without changing the selected entity, focused entity, active detail region, or inspection revision.
- Opening supported Page detail visibly requests bounded enrichment from the current immutable revision. The old revision remains visible while loading; `Esc` cancels; a late completion is not adopted; and only an explicitly returned revision may replace the current one.

### Semantic rendering contract demonstrated by the prototype

ANSI color and Unicode block glyphs are the primary presentation, but color never carries a dimension alone. Allocation, known occupied/free proportion, unknown occupancy, findings, focus, byte-region class, and slot-entry state also have a label, symbol, pattern, border, or textual value. Monochrome/ASCII mode preserves the same distinctions and navigation. Unknown occupancy remains different from zero occupancy.

The accepted Page example uses the exact 16,344-byte fragmented distribution asserted by `src/web.rs`: slotted header, live record extents, two fragmented-free intervals, contiguous free space, the complete slot directory, and allocated, unallocated, and deleted slot entries. Evidence labels remain structural and end in `bytes withheld`; the mock offers no payload, raw-byte, ciphertext, or secret display.

### Validation and remaining map state

The artifact was exercised with Playwright through click and keyboard descent/ascent, typed selector navigation, tier switching, loading cancellation, and all three structural variants. Headless captures cover Atlas at 120×36, Atlas monochrome/ASCII at 60×20, Ledger at 80×24, Workbench at 120×36, and Workbench loading at 60×20. The responsive-width assertion measured `939 > 626 > 433` CSS pixels for the 120-, 80-, and 60-column emulations, and no browser page errors occurred.

No new ticket is created by this resolution. The migration-shape fog still depends on the shared-projection and rendering-architecture decisions, while real terminal compatibility may yet expose a precise display-width, monochrome, fallback, or mouse-hit-region question. This prototype resolves the visual and interaction hierarchy only; it does not implement the production TUI or change the web viewer.
