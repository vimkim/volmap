Label: wayfinder:grilling
Type: grilling
Status: resolved
Assignee: codex
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Prototype terminal interaction parity across Volume, Sector, and Page](01-prototype-terminal-interaction-parity.md)

# Define the TUI navigation, focus, and history state model

## Question

What explicit TUI state machine governs the Volume → Sector → Page replacement hierarchy, breadcrumbs, `Enter` descent, `Esc`/`Backspace` ascent, back-stack restoration, roving page focus, independent pane scrolling, overlays, filters, findings navigation, mouse hit regions, resize, and existing sector/volume accelerators? Resolve which state survives screen changes and revision adoption, how inaccessible or filtered selections behave, and how keyboard-only and mouse operation remain equivalent at every supported terminal tier.

## Answer

### Decision

The user accepted all six recommendations and confirmed shared understanding. Introduce one deep `AtlasMachine` module inside the TUI inspection adapter, above the accepted Projection workspace and below terminal input and rendering. Its two-entry-point interface is the test surface for every key, mouse, resize, layout, progress, and completion path:

```rust
AtlasMachine::start(
    workspace: Arc<ProjectionWorkspace>,
    start: AtlasStart,
) -> Result<(AtlasMachine, AtlasStep), AtlasFault>

AtlasMachine::advance(
    &mut self,
    event: AtlasEvent,
) -> Result<AtlasStep, AtlasFault>
```

`AtlasStep` contains one immutable semantic `AtlasScene` for the renderer and ordered `AtlasEffect`s for the event loop to execute. Effects are initially `RunEnrichment` and `Quit`; Projection workspace reads occur inside deterministic transitions, while the event loop schedules its synchronous cooperative enrichment operation and returns typed progress/completion events. `crossterm`, terminal rectangles, ANSI/glyph choices, worker/channel types, HTTP, and browser history never cross this interface.

This closed reducer is preferred over a generic runtime statechart: the destination fixes exactly three replacement screens, so stringly view/command registries add interface without leverage. It is also preferred over an opaque `run()` runtime because rendering architecture and worker scheduling remain open downstream decisions.

### Canonical state

```rust
struct AtlasState {
    snapshot: SnapshotId,
    displayed: RevisionView,
    trail: AtlasTrail,
    filter: Option<NormalizedFilter>,
    finding: Option<DiagnosticOccurrenceId>,
    overlay: Option<Overlay>,
    enrichment: EnrichmentState,
    layout: LayoutState,
    notice: AtlasNotice,
}

enum AtlasTrail {
    Volume(VolumeFrame),
    Sector(VolumeFrame, SectorFrame),
    Page(VolumeFrame, SectorFrame, PageFrame),
}

struct VolumeFrame {
    volume: VolumeEntityId,
    focused_sector: SectorEntityId,
    scroll: ContentAnchor,
}

struct SectorFrame {
    sector: SectorEntityId,
    focused_page: PageEntityId,
}

struct PageFrame {
    page: PageEntityId,
    active_region: PageRegion,
    scroll: BTreeMap<PageRegion, ContentAnchor>,
}

enum PageRegion {
    Facts,
    Distribution,
    Slots,
    Chain,
    Findings,
    Coverage,
}
```

The **Atlas trail** is the typed Volume → Sector → Page ancestry plus each ancestor's restoration state. It is the back stack and the sole breadcrumb source; it is not chronological history. The one displayed exact Inspection revision sits outside the trail, so ascent can never resurrect an older revision and a scene can never mix revisions.

Focus is prospective; selection is committed into the trail. Volume focus names a Sector, Sector focus names one of its 64 Pages, and Page focus names an active semantic region or stable item within it. Identities and content anchors are semantic—Sector id, Page id, fact key, byte-region identity, Slot id, Diagnostic occurrence id, or coverage facet—never vector indexes, rendered labels, table names, terminal coordinates, or raw rows.

### State invariants

- The database snapshot never changes. Every Projection frame in one scene exactly matches `displayed`.
- A Page always belongs to the trail's Sector and Volume; a Sector always belongs to the trail's Volume. Breadcrumbs are derived from this valid trail and cannot disagree with the screen.
- Exactly one roving Entity-reference focus exists on Volume and Sector. `Enter` commits it as the next selected trail frame; merely moving focus never descends.
- Each Page region retains an independent semantic content anchor even when the current tier shows regions stacked or tabbed.
- Filters never delete, reorder, or disable physical topology. Every Sector card and every one of a Sector's 64 Pages remains present, focusable, and descendable.
- Unreadable, unsupported, encrypted-opaque, diagnostic-bearing, or not-yet-enriched Pages remain valid navigation targets. Their typed facts control enrichment availability, not existence.
- There is at most one modal overlay and one active enrichment request.
- Findings use typed Diagnostic occurrences and affected Entity references. Diagnostic subjects, codes, messages, table labels, and rendered strings are never parsed into navigation.
- Layout geometry is transient and generation-stamped. It never becomes navigation identity.
- Given the same initial exact revision and ordered event trace, the machine produces the same semantic scenes and effects without consulting a clock, randomness, or background mutable state.

### Hierarchy, history, and accelerators

- `Enter` on Volume pushes the focused Sector frame; `Enter` on Sector pushes the focused Page frame. Mouse activation first commits the clicked Entity as focus and invokes the same activation transition.
- `Esc` or non-editing `Backspace` pops exactly one Atlas-trail level after higher-priority dismissal rules. Page → Sector restores the exact Page focus; Sector → Volume restores the exact Sector focus and Volume content anchor.
- Activating a breadcrumb truncates the existing trail to that ancestor. It neither pushes a navigation record nor emulates browser Back/Forward.
- `/` and the existing `g` alias open the canonical Entity-selector editor. A valid `volume:`, `sector:`, or `page:` selector transactionally constructs that Entity's canonical trail at the named depth; failure changes no navigation state and leaves a typed reason in the editor.
- `[`/`]` move to the adjacent Sector without wrapping. At Volume they move Sector focus. At Sector they replace the selected Sector and preserve the focused Page's physical cell ordinal. At Page they replace Sector and Page with the same cell ordinal, retain Page depth and active region, and deactivate the old Page's enrichment before the new Page is considered. An edge key is a no-op with a notice.
- `PageUp`/`PageDown` retain their existing Volume-accelerator role: select the adjacent Volume without wrapping, replace the trail with its Volume screen, and focus its first Sector.
- Sector arrow movement is a true fixed 8×8 rover: left/right never cross row edges, up/down retain the column, edges clamp, and nothing wraps. Volume arrows use the current renderer-produced directional focus graph because card columns vary by tier; changing layout preserves the focused Sector identity.
- `n`/`N` traverse all typed Diagnostic occurrences in deterministic Entity/evidence-locus/occurrence order, independently of the visual filter, and wrap. Page-, Slot-, and OOS-scoped occurrences land on the containing Sector with the affected Page focused, so automatic Page enrichment still requires `Enter`. Sector and Volume occurrences land at their nearest Atlas level; snapshot/global or unresolved references open Finding details without manufacturing an Entity.
- Sibling, selector, and finding navigation constructs or replaces the canonical structural trail. There is no arbitrary chronological history and no TUI Back/Forward feature.

### Filters, regions, overlays, and dismissal

The normalized filter survives screen changes, ascent, sibling navigation, resize, and revision adoption. Nonmatches are dimmed in place. Applying a filter never moves focus; a directly selected nonmatch remains usable and is announced as outside the filter. Invalid filter changes are atomic no-ops. Finding traversal remains global rather than silently hiding diagnostics behind a visual lens.

`Tab`/`Shift-Tab` cycles Page regions in this fixed order and wraps:

```text
Facts → Distribution → Slots → Chain → Findings → Coverage
```

Preserve existing numeric meanings: `1` Facts/Structure, `2` Slots, `3` Chain, `4` Findings, `5` Coverage, and `6` About/licenses. Add `d` as the direct Distribution accelerator. `?` opens contextual controls help; `6` opens the About/licenses section of the same modal overlay. `j`/`k` and Page-screen vertical scrolling affect only the active region. `q` quits except while a text editor owns the keystroke.

There is one modal overlay: Entity selector, filter editor, Help/About, or Finding details. Input precedence is fixed:

1. In an editor, `Backspace` edits and `Esc` closes; ordinary global accelerators are text.
2. Help/About or Finding details closes.
3. Active enrichment receives cancellation and the Page remains displayed.
4. A later `Esc`/`Backspace` ascends one trail level.
5. At Volume, navigation state remains unchanged and the root is announced.

Breadcrumb, selector, sibling, finding, or Volume navigation away from an enriching Page deactivates and cancels that request before committing the new trail. Resize and filter changes do not cancel it.

### Scroll, resize, and keyboard/mouse equivalence

At 120×36 and larger, Facts and Distribution panes scroll independently and Slots continues with its own anchor. At 80×24, the stacked workspace maps its visible position to `{ region, semantic item }`; at 60×20, each tab restores its own anchor. A round trip across tiers restores the independent wide-pane anchors rather than copying a raw row offset between layouts.

Resize preserves the displayed Inspection revision, Atlas trail, every Entity focus, filter, finding cursor, active Page region, semantic anchors, overlay text, and enrichment request identity. It invalidates numeric extents and mouse regions, recomputes tier/layout, resolves each anchor to the nearest surviving predecessor, and then clamps. Below 60×20, the machine retains state behind a reversible too-small scene that accepts resize, quit, and cancellation; it does not exit and discard the session.

The renderer consumes `AtlasScene` and returns a `LayoutCommit` containing its extent, generation, directional focus graph, scroll extents, and semantic `ControlId`/`ScrollRegion` hit regions. A resize advances the generation before another pointer action may resolve. Stale-generation clicks are ignored; equal-precedence regions are clipped and non-overlapping.

Keyboard and mouse adapters produce the same semantic actions. Every hit-region action has a keyboard path; clicking an Entity uses the same activate transition as focusing it then pressing `Enter`. Mouse wheel scrolls only the region under the pointer and makes it active. Hover is non-semantic.

### Enrichment and revision adoption

The navigation module owns only adapter request identity and visible state; Projection workspace owns admission, cancellation semantics, and immutable publication. An automatic request records its unique id, target, exact base revision, cancel token, and typed progress. The old revision remains displayed during work.

Only a completion whose request id, target, snapshot, and base revision still match the active Page request may offer automatic adoption. Adoption is one transaction:

1. Checkout the explicitly returned exact `RevisionView`.
2. Reproject and validate the complete current Atlas trail against it.
3. Replace the one global `displayed` handle only if the whole trail is valid.
4. Drop projection/layout caches and install a new layout generation.
5. Preserve trail identities, focus, filter, finding occurrence identity, active region, overlay, and semantic anchors; clamp only anchors whose item disappeared.

The trail can therefore never contain mixed revisions. A failed candidate retains the complete old scene. Matching typed errors retain the old revision and route and become notices.

User input is ordered before a simultaneously ready worker completion. Cancellation or navigation deactivates automatic adoption immediately; a later publication may remain in the Projection workspace and produce a notice, but never navigates or adopts. The detailed progress, cancellation-race, offered-revision, and retry affordances remain for [Define automatic enrichment and immutable-revision transitions](05-define-enrichment-revision-lifecycle.md) within these invariants.

### Compatibility gates

Replace the current handler-private tests with behavior tests through `AtlasMachine::start/advance`:

- Replay Volume → Sector → Page → Sector → Volume and prove exact Sector/Page focus and semantic scroll restoration; prove breadcrumb truncation equals repeated ascent.
- Assert the fixed 8×8 Page rover at every edge and renderer-supplied Volume focus graphs at 120×36, 80×24, and 60×20.
- Replay paired keyboard and mouse traces for every enabled semantic control and assert identical trail, focus, filter, region, scroll, and effects. Reject stale-generation clicks after resize.
- Preserve `[`, `]`, `PageUp`, `PageDown`, `/`, `g`, `f`, `n`, `N`, `1`–`6`, `Tab`, `Shift-Tab`, `j`, `k`, `?`, and `q` semantics, plus the new `d` Distribution shortcut.
- Prove filters dim without removing topology, retain now-nonmatching focus, and allow selector navigation to a filtered Entity. Invalid selectors/filters must preserve the prior state atomically.
- Drive finding navigation with typed Diagnostic occurrences, deliberately misleading messages/subject strings, deterministic wrap, Slot/OOS ancestor landing, and unresolved-reference handling.
- Prove every Page region restores its independent anchor across scrolling, region changes, descent/ascent, tier changes, and revision adoption. Exercise temporary 59×19 suspension and recovery.
- Assert opening eligible Page detail emits exactly one request at the displayed exact revision; unavailable, unsupported, opaque, and already-complete Pages emit none.
- Exercise matching adoption, cancellation, late/wrong-request/wrong-snapshot/stale-base completion, diagnostic-bearing and Invalidated revisions, and user-input-before-completion ordering. No trace may mix revisions or adopt without an active match.
- Preserve Page/Sector file/class/table attribution from `cba72cd` as opaque typed projection content; label changes must not change navigation identity.
- Property-test arbitrary event traces for valid Atlas ancestry, one displayed revision, unique focus, one overlay/request, bounded resolved scroll, deterministic effects, and layout-generation safety.
- Keep terminal entry/cleanup tests for TTY/raw-mode/alternate-screen restoration. Replace fixed-coordinate tab tests, diagnostic-subject parsing tests, and character-count truncation tests with semantic action traces; display-cell width and terminal-control sanitization are explicit gates of [Define semantic color, glyph, and fallback mappings](06-define-semantic-terminal-rendering.md).

No production TUI is implemented by this decision. No new ticket is created: the precise source-derived label sanitization concern is added to the existing semantic-rendering ticket, and migration shape remains fog until the rendering architecture is chosen.
