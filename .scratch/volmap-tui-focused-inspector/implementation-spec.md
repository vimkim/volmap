# Volmap focused TUI implementation specification

Status: implemented

Delivery: [Cut over and remove the legacy TUI](issues/06-cut-over-and-remove-legacy-tui.md)

Source baseline: `6e2c8ae` (`build: add reproducible frontend foundation`)

Delivery prerequisite: [Establish the React compatibility viewer](../volmap-live-web-runtime/issues/02-establish-react-compatibility-viewer.md) must land first so the live browser and its semantic regression harness are stable.

Supersedes: the full [terminal-parity implementation specification](../volmap-tui-web-parity/implementation-spec.md) and its 13 implementation tickets. The completed Wayfinder map remains historical design evidence; this specification is the current TUI delivery authority.

## Product boundary

The TUI is a focused storage inspector with exactly three persistent modes:

1. **Volume** shows a scrollable mosaic of Sector cards. Every card contains all 64 physical Pages as compact occupancy marks.
2. **Sector** shows one Sector's 64 Pages with exact occupied/free percentages and enough type, allocation, finding, and attribution context to choose a Page.
3. **Page** shows one Page's exact structural byte distribution. Live record extents are selectable. `Enter` on a selected record shows its existing record interpretation as Page-local detail; `Esc` returns to the same record.

The web viewer remains the richer workspace for browser history, Slot/OOS routes, runtime observations, cross-highlighting, and dense schema exploration. The TUI does not promise web interaction parity and does not start a server, launch a browser, or construct web URLs.

Filters, global finding traversal, Chain/Coverage/About tabs, dedicated Slot/OOS screens, runtime overlays, arbitrary browser navigation, and a general terminal widget framework are outside this focused delivery. Diagnostics, coverage/outcome, attribution, and structural limitations remain visible in the title, status, focused descriptor, or Page detail when they affect the active facts.

## Occupancy vocabulary

Volume uses a two-column Page microcell: one allocation token followed by one occupancy mark. Allocation is `S` system metadata, `A` allocated, `R` reserved-unallocated, or `U` unreserved. The mark is never the sole carrier of allocation or findings.

Known positive occupancy uses a bottom-up Unicode Braille fill with eight levels:

```text
1/8 ⡀   2/8 ⣀   3/8 ⣄   4/8 ⣤   5/8 ⣦   6/8 ⣶   7/8 ⣷   8/8 ⣿
```

The level is `ceil(occupied_percent × 8 / 100)`, clamped to 1–8. Known zero is `0`, unknown is `?`, and explicitly not-applicable is `-`; these states never collapse. ASCII fallback uses `1`–`8`, `0`, `?`, and `-`. A finding adds a separate `!` marker in the card heading or focused descriptor rather than replacing occupancy.

Sector mode prints the exact projected occupied percentage in every Page cell. At 80 columns and wider it also shows the compact physical type; at 60–79 columns the focused descriptor carries type, allocation, exact occupied/free percentages, finding, and file/class/table attribution while the grid keeps every exact percentage. Unknown and not-applicable remain textual `?` and `-`, never `0%`.

## Interaction model

The state is a structural path, not chronological history:

```text
Volume { focused_sector, top_sector }
Sector { sector, focused_page }
Page   { page, selected_distribution_item, top_distribution_item,
         interpretation: closed | record(record_oid, top_attribute) }
```

- Arrow keys move semantic focus. Volume movement follows the packed card grid; Sector movement is a clamped 8×8 rover; Page movement visits exact byte regions while record-selection commands skip non-record regions when requested.
- `Enter` descends Volume → Sector → Page. In Page mode it opens the selected live record's interpretation, enriching first when needed.
- `Esc` or `Backspace` closes record interpretation, then ascends Page → Sector → Volume. Each ascent restores the exact child focus and scroll anchor.
- Mouse activation and wheel scrolling, when enabled, invoke the same semantic actions as keyboard input. Core operation remains complete from the keyboard.
- `[`/`]` select adjacent Sectors without wrapping, `PageUp`/`PageDown` select adjacent Volumes, `?` shows the small contextual key legend, and `q` quits.
- Resize retains mode, exact revision, identities, selected record, interpretation state, and semantic anchors. Below 60×20 it shows a reversible too-small notice rather than exiting.

## Page distribution and record interpretation

Page mode consumes one shared presentation-neutral Page distribution derived only from a validated slotted Page. The shared value covers all 16,344 content bytes exactly:

- header `[0, 32)`;
- every live record extent ordered by `(offset, slot_id)`;
- every fragmented and contiguous free interval;
- the complete trailing four-byte Slot directory; and
- every Slot entry with its Slot id, entry range, record type, and allocated, empty, or deleted state.

The screen combines a proportional byte bar with exact scrollable rows. Header, free, and directory rows are visible but not interpretable records. A live record row is selectable by its stable OID/Slot identity. Tombstones and empty entries cannot trigger interpretation.

`Enter` on a live record uses the existing page-granularity record enrichment. It resolves the Page's class representation once, interprets every supported home record on the Page, follows the already-supported one-hop relocation behavior, and publishes one immutable revision. `REC_BIGONE`, unsupported types, malformed evidence, encrypted-opaque data, and root/system heaps show their typed structural fact or durable reason rather than an empty panel or raw bytes.

The Page-local interpretation detail shows record identity/type, class/table, representation id, relocation origin when present, record-layout regions, and the ordered attribute name/domain/state/value projections already governed by explicit-target disclosure. Typed decoded values may appear only after this explicit record action. Undecodable and unrequested bytes remain withheld.

## Enrichment and revision adoption

The TUI owns one current immutable `GraphView`, one bounded `ResourcePolicy`, and at most one worker. It uses existing inspection operations instead of introducing the superseded Projection workspace:

- entering Page automatically requests Page enrichment only when structural distribution is supported and absent;
- `Enter` on a live record requests record-page enrichment only when the selected interpretation is absent and no durable Page-level failure is already present;
- the old exact revision remains displayed with a loading notice while work runs;
- `Esc`, ascent, sibling navigation, or quit cancels and deactivates the request before changing context; and
- only a completion matching the active request, snapshot, exact base revision, Page, and selected record may replace the current `GraphView`.

Adoption re-resolves the complete current path and selected OID against the returned view before replacing the displayed revision. Failure retains the old view and shows a typed reason. Late or nonmatching results are dropped; the focused TUI has no revision-offer history, latest lookup, background queue, or automatic retry. A successful adopted view is returned to the CLI-facing caller so final outcome handling is not frozen at the pre-TUI revision.

## Module seams

Use three focused seams behind small interfaces:

- **Inspection/projection** keeps enrichment and semantic derivation below adapters. The existing Inspection module gains one shared bounded record-selection recipe that establishes Page structure, relocation evidence, and Page-granularity interpretation; shared projection owns Page distribution geometry plus the existing Page, Slot, and interpretation values. Web and TUI consume these operations and facts without duplicating them.
- **TUI session** owns the current immutable view, three-mode path, focus/anchors, one request identity, cancellation, matching adoption, and semantic actions. Its private worker calls existing synchronous inspection enrichment with the caller's policy.
- **Terminal renderer/host** owns responsive placement, Braille/ASCII encoding, safe display-column text, semantic hit regions, frame presentation, TTY/raw/alternate-screen/mouse/cursor lifecycle, and cleanup over the pinned Crossterm dependency.

The CLI hands the TUI an owned initial view and resource policy and receives a typed exit containing the final adopted view. Crossterm events, coordinates, worker channels, raw volume bytes, web state, and HTTP types stay outside the TUI session interface.

Do not add Ratatui or a generic widget/state framework. The implementation needs three fixed screens, one Page-local detail, and two presentation profiles: primary ANSI/Unicode and monochrome/ASCII fallback. Source-derived labels pass through one control-sanitizing, grapheme-safe, display-column-aware text path before placement.

## Resource and compatibility contract

- Volume projection and painting are viewport-bounded and never sample topology. Every visible Sector card always contains its complete 64 Pages; scrolling and direct sibling movement can reach every Sector.
- In addition to the immutable `GraphView`, retain only visible Sector-card rows plus one row of overscan, one current Page distribution, one selected record interpretation, the current and previous terminal frame, and one pending worker completion. Retained presentation state does not grow with total Sector count.
- Page rows are formatted only for the visible window plus one screen before and after. Every exact distribution row remains reachable.
- Interactive input is never dropped. Resize may coalesce; input is handled before a simultaneously ready completion. There is no idle redraw or animation.
- Cache pressure and terminal size never change Inspection coverage, diagnostics, outcome, or disclosure. Unsupported and unavailable facts render as facts rather than terminal errors.
- Preserve every merge-base web, React, CLI, JSON/JSONL, deterministic HTML, inspection, record-interpretation, disclosure, static-musl, notice, SBOM, and reproducibility behavior. Moving Page distribution into a shared projection must leave web JSON and browser behavior unchanged.

## Verification

Acceptance evidence stays proportional to the focused scope:

- shared tests for exact 16,344-byte distribution conservation, ordering, free intervals, Slot states, and unchanged web serialization;
- deterministic TUI-session traces for descent/ascent restoration, 8×8 Page focus, record selection, resize, cancellation, matching adoption, late completion, invalidation, and quit;
- twelve core semantic cell/geometry goldens: Volume, Sector, Page, and interpretation at 120×36 ANSI/Unicode, 80×24 ANSI/Unicode, and 60×20 monochrome/ASCII; focused structural cases cover loading, failure, alternate profile/tier combinations, and reversible 59×19 suspension without another Cartesian snapshot set;
- exhaustive occupancy tests for known zero, existing 7/93, positive buckets through 100, unknown, and not-applicable, proving Volume marks and Sector percentages agree with one projection;
- record fixtures covering home/newhome, relocation, unsupported `REC_BIGONE`, tombstones, partial attributes, Page-level interpretation failure, and explicit-target disclosure;
- renderer/property tests for hostile labels, controls, Unicode width, clipping, complete focus/hit geometry, bounded rows, and failed-frame non-adoption;
- scripted-host and PTY tests for keyboard/mouse equivalence, resize, worker ordering, cancellation, normal/error cleanup, and reversible 59×19 recovery; and
- all merge-base adapter and release tests after the focused production cutover.

Screenshots and raw ANSI transcripts are review aids. Stable assertions are typed facts, session state/effects, normalized semantic cells, committed geometry, reachability, and revision/disclosure behavior.

## Dependency-ordered delivery

```text
React compatibility viewer
          |
          v
F1 Volume overview -> F2 Sector detail -> F3 Page distribution
                                            |
                                            v
                                  F4 Record interpretation
                                            |
                                            v
                                  F5 Harden and cut over
```

### F1 — Volume overview

Replace the legacy single-Sector start screen in a non-production TUI construction with the complete scrollable Volume mosaic, two-column Page microcells, Braille/ASCII occupancy, responsive packing, focused descriptors, and bounded visible rows.

Completion: exact occupancy/allocation/finding/attribution facts render for every visible complete Sector; all Sectors remain reachable; Volume semantic traces and goldens pass without changing production `tui::run`.

### F2 — Sector detail

Add Volume → Sector descent and the detailed 64-Page Sector screen with exact percentages, clamped focus, responsive type/context, sibling movement, mouse equivalence, and exact ascent restoration.

Completion: every Page is visible and reachable once, exact projected percentages agree with Volume marks, and all Sector interaction/profile/resize tests pass.

### F3 — Page distribution

Extract the validated Page distribution into shared projection, add bounded automatic Page enrichment, and deliver the Page byte bar plus exhaustive scrollable rows with live-record selection.

Completion: shared geometry and unchanged-web gates pass; Page entry/loading/cancellation/adoption is revision-safe; every region remains reachable while formatting stays viewport-bounded.

### F4 — Record interpretation

Connect `Enter` on a selected live record to page-granularity interpretation enrichment and show the Page-local interpretation detail with stable record identity, explicit-target disclosure, typed partial/failure behavior, and exact return focus.

Completion: all supported, relocation, unsupported, failure, cancellation, stale-result, and disclosure fixtures pass without exposing interpretation for empty/deleted/non-record regions.

### F5 — Harden and cut over

Complete terminal text safety, renderer/host lifecycle, bounded scheduling, mouse parity, resource assertions, merge-base compatibility, and production cutover. Delete the legacy state/draw path after the focused evidence passes.

Completion: production `tui::run` uses only the focused session/renderer/host, returns its final adopted view, all focused and merge-base gates pass on one candidate, and no full-parity Atlas/Projection-workspace implementation is introduced.
