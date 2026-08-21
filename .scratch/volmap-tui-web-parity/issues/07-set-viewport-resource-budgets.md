Label: wayfinder:grilling
Type: grilling
Status: resolved
Assignee: codex
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Prototype terminal interaction parity across Volume, Sector, and Page](01-prototype-terminal-interaction-parity.md), [Choose the terminal rendering architecture and dependency boundary](04-choose-rendering-architecture.md)

# Set volume viewport and rendering resource budgets

## Question

What bounded viewport, caching, and redraw policy lets the TUI navigate complete large-volume mosaics while keeping input latency and memory predictable? Decide sector-card packing and scrolling, visible-window and overscan rules, resize invalidation, page-distribution row virtualization, stable focus across window changes, redraw triggers during enrichment, and measurable resource/latency budgets. Distinguish terminal rendering budgets from existing inspection operational budgets and never silently sample or omit sectors.

## Working agreement

Resolve this ticket on branch `wayfinder/tui-viewport-budgets` in the sibling
worktree `/home/vimkim/temp/volmap-tui-viewport-budgets`. Keep `main` unchanged
while the decision is being developed. After the user confirms the complete
shared understanding and the ticket resolution passes its verification gates,
squash-merge the branch to local `main` as one commit. Tickets 08 and 09 remain
unclaimed and unmodified during this session.

## Answer

### Decision

The user accepted every recommendation over two decision rounds and confirmed
the complete shared understanding. The TUI keeps the interfaces accepted by
tickets 03 and 04: `AtlasMachine::start`/`advance` remains the semantic state
and projection boundary, and `AtlasRenderer::compose` plus
`PreparedFrame::present` remains the rendering boundary. Ticket 07 adds no
public viewport manager, cache protocol, paging token, renderer callback, or
workspace handle.

The chosen policy is a **bounded private reservoir with a row-windowed Atlas
mosaic**:

- `AtlasMachine` owns one contiguous, exact-revision reservoir of at most 64
  complete Sector-card projections, together with typed focus and content
  anchors.
- `AtlasRenderer` owns deterministic card packing, the visible and overscan
  rows, detail-row virtualization, width-dependent presentation caches, cell
  frames, damage calculation, and committed hit geometry.
- The terminal host owns one dirty-frame scheduler and supplies input, resize,
  monotonic timing, and presentation. It does not learn projection windows or
  cache keys.
- Every Sector remains addressable in physical order. Bounding projection and
  painting never means sampling, decimation, aggregation, or omission from the
  navigation topology.

This was selected over two alternatives. A fixed 64-Sector display page makes
the window visible to the operator and grossly overfetches at 60x20. A
renderer-driven `FrameDemand` handshake projects exactly the required rows but
widens `compose`, creates a fetch/recompose cycle, and makes the event loop
participate in an otherwise private concern. The accepted reservoir gives the
common caller one semantic event and at most one frame while retaining bounded
local refills and deterministic memory.

### Prototype evidence

The throwaway comparison is isolated on branch
`prototype/tui-viewport-budgets` at commit
`c7e26b82225d60c3b4aec5c956dbf19b7c314a6c`. Open the
[interactive viewport prototype](/home/vimkim/temp/volmap-tui-viewport-prototype/prototype/tui-viewport-budgets.html)
with `?variant=A`, `B`, or `C`; captures live beside it under
`prototype/captures/`.

Variant A, visible rows plus one overscan row on each side, is accepted.
Variant B's fixed 64-Sector pages visibly overfetch and spend scarce compact
space on paging controls. Variant C's three-row runway and LRU-style telemetry
add state without a demonstrated navigation benefit. The prototype models a
complete topology up to 33,554,432 Sectors while materializing only the
window; it is design evidence, not production code, and its branch does not
enter `main`.

Ticket 01's early visual mock used narrow preview cards. Ticket 06 is the later
and normative semantic constraint: eight seven-column Page strips require 56
content columns. This ticket therefore deliberately replaces the prototype's
narrow-card packing with the fixed extent below.

### Terminal rendering budget

A **Terminal rendering budget** is an adapter-local ceiling on active terminal
cells, exact-revision projection windows retained by Atlas, prepared detail
rows, presentation caches, redraw cadence, and frame latency. It is distinct
from Inspection's `ResourcePolicy` and operational budget.

Reaching a presentation cache limit evicts or recomputes presentation state.
It never changes Inspection coverage, outcome, availability, diagnostics, or
the set of entities that can be reached. An impossible mandatory bounded frame
is an internal `RenderFault::BudgetInvariant`, not a resource-limit diagnostic
and not permission to sample or truncate facts.

The hard incremental terminal-rendering memory ceiling is **16 MiB** per Atlas
session. It includes the reservoir references and Atlas-owned summaries, the
current prepared frame and last successfully presented frame, visible-row
fragments, geometry, sanitized-text caches, and width-dependent rasters. It
excludes immutable revision storage already owned by the Projection workspace
and terminal/OS write buffers. Allocation-accounting tests must enforce both
the byte ceiling and constancy with respect to total Volume size and progress
event count.

### Volume mosaic geometry and complete topology

The active logical canvas is clamped to **256x128 cells**, or 32,768 cells.
Larger physical terminals clear their unused margins and center the bounded
canvas; those margins contain no controls or hit regions. Supported smaller
surfaces use their full extent. Below 60x20, Atlas retains state in ticket 04's
reversible too-small scene rather than attempting a partial parity layout.

One Sector card is **58x11 cells**:

- 56 content columns for the complete 8x8 raster of ticket 06's seven-column
  Page strips;
- two border columns; and
- a heading row, eight Page rows, and top/bottom framing within 11 rows.

A one-column and one-row gutter gives a **59x12 packing stride**. Complete cards
pack row-major in physical Sector order. For a content rectangle that can hold
at least one card:

```text
columns      = floor((content_width  + 1) / 59)
visible_rows = floor((content_height + 1) / 12)
```

The renderer never paints a partial card and never introduces horizontal
mosaic scrolling. The accepted 120x36, 80x24, and 60x20 tiers consequently
pack 2, 1, and 1 card columns when their Volume content widths equal the tier
widths. At the maximum canvas, at most four columns by ten rows are visible.

The materialized display window is every complete visible card row plus
exactly one complete row before and after. At the maximum canvas that is at
most 48 cards, so one 64-Sector reservoir always covers a valid first
composition without a renderer-to-workspace demand round. First and last
partial logical rows clamp normally.

The Volume scene carries total Sector count and checked physical ordinals; it
does not allocate an adjacency graph or projection per Sector. Arrow topology,
direct selectors, finding jumps, first/last movement, and row calculations use
checked ordinal arithmetic. The fully Page-id-addressable upper fixture is
33,554,432 Sectors, 2,147,483,648 Pages. A larger raw header count that cannot
be represented by the existing signed Page identity is an existing
format/projection issue, not something the renderer hides or repairs.

Wheel movement advances one complete card row. Page movement advances one
visible window minus one row so context remains. Direct navigation computes
the destination row rather than walking intervening Sectors. Every valid
Sector ordinal can therefore become visible and focused without resident work
growing with the Volume.

### Reservoir, focus, and resize

The reservoir key is `(snapshot, exact revision, Volume identity)` and its
contents are ascending, gap-free Sector ordinals. Every Sector card contains
all 64 Page summaries. Projection requests are contiguous and individually
bounded to 64 Sectors.

Nearby scrolling reuses overlap and projects only a missing prefix or suffix.
A distant selector/finding jump atomically replaces the reservoir around the
target. Changing Volume, adopting a revision, or invalidating the snapshot
drops it. There is no multi-Volume, cross-revision, or history LRU. A cache miss
is ordinary recomputation. A failed query leaves the last successfully
presented scene, focus, revision, and reservoir installed and presents the
typed notice through normal Atlas state; a wrong-revision, gapped, out-of-range,
or non-64-Page Sector result is an internal invariant fault and is never
partially installed.

Focus and scroll anchors are typed Sector identities, not viewport indexes.
Keyboard focus movement minimally reveals the destination with a complete
card. Wheel scrolling changes only the content anchor and may intentionally
leave focus offscreen; the status identifies the focused Sector and shows its
up/down direction. `Enter` still activates that explicitly named focused
Sector. Selector and finding navigation focus and reveal their exact target.

On the first resize event, Atlas immediately invalidates the installed layout
generation so pointer events against it become stale no-ops. It then:

1. retains exact revision, Volume, semantic focus, and top Sector anchor;
2. coalesces queued resize events to the newest extent;
3. aligns the prior anchor to its nearest predecessor under the new column
   count;
4. keeps focus visible with the smallest shift if it was visible before the
   resize, but does not reveal focus that was already intentionally offscreen;
5. clamps at the first or last logical row; and
6. activates new hit regions only after the complete frame flush succeeds.

Wide -> compact -> too-small -> wide therefore preserves identity and semantic
anchors even though numeric rows and columns are rebuilt.

### Page-row virtualization

The shared Page projection remains one atomic, exact, exhaustive value.
Virtualization bounds only terminal measurement, formatting, geometry, and
painting. It never changes the Page byte map, Slot order, byte conservation,
or availability of a row.

Distribution and Slot entries are fixed one terminal row each. The renderer
prepares the visible rows plus one full viewport before and after, capped at
**384 prepared rows** across the active Page workspace. Each row is keyed by a
typed semantic identity such as byte-region identity, Slot id, Diagnostic
occurrence, or coverage facet. Scroll state also uses that identity or its
nearest valid predecessor, never a cached vector index.

Narrow layouts clip fields through the ticket 06 `TerminalText` policy instead
of eagerly wrapping thousands of rows; the focused descriptor exposes the
complete sanitized label, range, and value. The proportional byte raster is
computed once for the current `(revision, Page, region width, presentation
profile)` and still covers the complete 16,344-byte content space.

The source-derived worst cases are 4,078 Slot rows and a conservative 8,160
Distribution rows. The first, last, and every intermediate row remain
scrollably reachable while prepared-row work stays bounded by the viewport.

### Cache invalidation

Invalidation is deterministic and does not rely on eviction timing:

| Change | Required invalidation |
| --- | --- |
| Snapshot invalidation or replacement | Reservoir, Page projection, all presentation caches, prepared/prior frames, layout generation |
| Exact-revision adoption | Reservoir, Page projection, all content-derived fragments, prepared/prior frames, layout generation |
| Volume replacement | Sector reservoir and Volume fragments |
| Presentation-profile change | Encoded text, glyph/style fragments, rasters, geometry, prepared/prior frames |
| Tier or width change | Placement, width-dependent rows and rasters, geometry, prepared/prior frames |
| Height-only change | Placement, scroll extents, geometry, prepared/prior frames; compatible sanitized text may remain |
| Filter change | Affected semantic fragments and scroll extents |
| Focus, selection, status, or progress | Only the presentation regions owned by that state |
| Scroll within the same window | Retain overlap; replace newly exposed edge fragments |

Revision-pinned projection facts may survive a resize only while their exact
key and reservoir range remain valid. No content-derived fragment crosses an
adopted revision. The presenter's prior-frame cache advances only after a
successful complete flush; a failed present emits no `LayoutCommit` and leaves
the last successful frame authoritative.

### Redraw scheduling and ordering

The terminal host maintains one dirty frame, never a queue of frames. There is
no idle redraw or animation timer.

- All key and mouse input remains ordered and is never coalesced or dropped.
- When input and a worker completion are simultaneously ready, input is
  reduced first, preserving ticket 05's cancellation authority.
- Drain at most 32 immediately ready input events before presenting so neither
  input nor painting starves.
- Interactive presentation is capped at 60 Hz. Multiple semantic changes
  inside one 16 ms interval retain their ordered state transitions but compose
  only the newest complete scene.
- Resize invalidates generation immediately and retains only the newest extent
  for a frame no later than the 16 ms coalescing boundary.
- Enrichment progress uses ticket 05's capacity-one newest-valid mailbox and
  presents at most 10 Hz. Interactive input absorbs pending progress into the
  same frame.
- Matching completion, cancellation, revision adoption, typed faults,
  overlays, and navigation are eligible for the next interactive frame and are
  never delayed behind progress throttling.
- Poll timeouts, stale worker results, stale pointer events, duplicate
  progress, clamped no-op movement, and an ordinary `LayoutCommitted` event do
  not dirty the frame.

One render cycle drains/coalesces events, reduces every semantic transition,
composes one complete off-screen frame from one immutable scene and revision,
validates its bounds and budget, flushes it, and only then installs its frame
cache and layout commit. Presentation never triggers enrichment by itself.

### Measurable budgets

The normative deterministic limits are:

| Resource | Hard limit |
| --- | ---: |
| Active logical canvas | 256x128 = 32,768 cells |
| Exact-revision Sector reservoir | 64 complete Sectors = 4,096 Page summaries |
| Visible Volume materialization | Complete rows plus one row each side; at most 48 cards at the canvas cap |
| Prepared Page rows | At most 384 |
| Atlas-owned incremental live heap | 16 MiB |
| Pending frame state | One dirty frame; current prepared and last successful cell frames only |
| Ready input drain | 32 events per cycle |
| Interactive presentation | At most 60 Hz |
| Progress presentation | At most 10 Hz, newest valid value only |
| Idle presentation | Zero |

Release-mode benchmarks use a recording presenter on a documented pinned
x86-64 reference host and report the sample count, toolchain, CPU, and complete
latency distribution. They gate:

| Path | Required latency |
| --- | ---: |
| Cold maximum-canvas composition | p99 <= 25 ms |
| Warm focus or one-row scroll composition plus diff | p95 <= 8 ms; p99 <= 16 ms |
| Input receipt through successful `LayoutCommit`, excluding terminal write/flush blocking | p95 <= 33 ms; p99 <= 50 ms |

Warm distributions use at least 10,000 iterations and cold distributions at
least 500. A controlled local PTY benchmark separately reports key-read through
successful flush so terminal backpressure cannot masquerade as projection or
composition time. Heterogeneous ordinary CI gates deterministic cardinalities
and operation counts rather than flaky wall-clock percentiles; the controlled
performance job owns the timing thresholds.

### Compatibility and verification gates

Tests exercise the accepted public Atlas seams and exact Projection fixtures,
not private cache helpers alone:

- Use a lazy synthetic 33,554,432-Sector source and jump to first, middle, and
  last positions. Assert that queries are contiguous and at most 64 Sectors,
  resident cardinality is constant, arithmetic does not overflow, and no
  operation walks total Volume size.
- Exhaustively traverse a 257-Sector synthetic Volume in both directions.
  Assert that the union of exposed windows is exactly every Sector in physical
  order, every card contains all 64 Pages, and no identity is sampled or
  substituted.
- Retain the real 4,096-Sector sparse fixture as an integration check while
  keeping inspection-resource accounting separate.
- Exercise 60x20, 80x24, 120x36, 256x128, adversarial `u16` terminal extents,
  and `120x36 -> 59x19 -> 256x128`. Assert the canvas/card limits, focus and
  anchor restoration, reversible too-small state, and stale-generation click
  rejection.
- Cross reservoir edges with arrows, wheel, page movement, selectors, and
  finding navigation. Nearby movement fetches only missing contiguous ranges;
  distant movement replaces atomically; query failure preserves the prior
  scene.
- Render the maximum 4,078-Slot and 8,160-Distribution-row fixtures. Assert
  that no more than 384 rows are prepared while every semantic row is
  reachable exactly once and the 16,344-byte distribution still conserves
  bytes.
- Repeat the tier, profile, filter, Volume, snapshot, and revision transitions
  in the invalidation table. Assert no old-revision projection, text fragment,
  geometry, or prior diff frame survives adoption.
- Feed at least 100,000 valid progress updates interleaved with input,
  cancellation, completion, and stale results. Assert capacity-one storage,
  at most ten progress frames per second, ordered input-first reduction,
  immediate terminal completion, and no redraw for ignored state.
- Allocation-count construction, steady scrolling, worst-case Page rows,
  full repaint, overlays, and revision adoption. Assert the 16 MiB ceiling and
  identical retained cardinality for small and maximum Volume topologies.
- Retain ticket 04's atomic-frame, short-write/flush failure, hit-region,
  terminal cleanup, presentation-profile, deterministic artifact, notice/SBOM,
  static musl, and web-unchanged gates. Ticket 06's semantic goldens remain
  authoritative inside every visible Sector card.

These gates supplement rather than redefine Inspection's existing resource
benchmark. Cache pressure and terminal slowness must never create Inspection
diagnostics, partial coverage, or a different exported revision.

No new ticket is created by this resolution. Ticket 08 owns the complete
cross-adapter verification contract, and ticket 09 assembles these accepted
decisions into the implementation specification.
