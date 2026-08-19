Type: prototype
Status: resolved
Blocked by: 04, 07, 13

# Prototype the web sector and slotted-page explorer

## Question

What web interaction and visual hierarchy makes a large CUBRID snapshot understandable without overwhelming the user? Build a cheap but interactive prototype covering database/volume overview, virtualized sector navigation, the selected sector's 64-page grid, page classification and ownership legends, and a selected-page workspace with both a physical page byte map and logical slot table. Include recognized page-type overlays and clickable OOS value-chain traversal, anomaly states, loading states, keyboard access, and the bounded raw-hex gate. Work with the user against concrete normal, sparse, fragmented, corrupt, and OOS-heavy scenarios; link the prototype asset from the resolution.

## Comments

### Standing human disposition

On 2026-08-19 the user directed every remaining ticket to accept the source-backed recommended option and continue without further HITL. The prototype will therefore compare three materially different layouts against the accepted model/security/decoder constraints, select the strongest combination by explicit criteria, and record that recommendation as accepted.

The earlier question's “bounded raw-hex gate” is superseded by the resolved web contract: version one has no raw-byte or hexadecimal endpoint under any authorization. The prototype must use page byte maps, structural extents, and evidence locators only.

### Prototype artifact and validation

The throwaway implementation lives on branch `prototype/web-explorer` at commit `7130feca863fb01faaa61fd20f29739d04ce995c`. Open the [interactive prototype](/home/vimkim/temp/volmap-web-prototype/prototype/web-explorer.html) from its isolated worktree; its adjacent README records the local launch command and controls. The branch is intentionally not merged into `main`.

One route exposes three materially different hierarchies through `?variant=A|B|C`:

- **A — Atlas:** persistent snapshot/volume/sector hierarchy, a virtual sector window, a 64-page grid, and a persistent selected-page inspector;
- **B — Focus:** summary cards, a horizontal sector ribbon, a large focus canvas, and a page drawer; and
- **C — Command:** typed navigation, a map-first page canvas, and a lower structure/relationship workspace.

All three exercise normal, sparse, fragmented, corrupt, and OOS-heavy scenarios plus a loading skeleton. They include keyboard page/variant navigation, page-type and allocation legends, selected/finding states, structural byte extents, slot/chain tabs, OOS traversal, canonical contained diagnostics, evidence locators, and an explicit “bytes withheld” boundary. There is no network dependency, raw-byte control, hexadecimal control, or payload-valued mock data.

Validation on 2026-08-19 covered JavaScript syntax, local HTTP delivery, and headless Chrome renders of all three variants at 1440×1000 plus Atlas at 1024×768 and 390×844. The desktop variants rendered without console-blocking failures. Atlas preserves the full three-pane workflow at desktop, moves the inspector below the map at tablet width, and retains touch-scrollable sector/page regions at phone width.

### Accepted prototype verdict

Under the standing disposition, the accepted production direction is **Atlas as the information architecture**, with two selectively borrowed affordances:

1. Retain Atlas's persistent snapshot/volume hierarchy, virtualized sector window, fixed 64-page sector grid, and context-preserving page inspector. It is the only variant that keeps database, volume, sector, page, and relationship context simultaneously legible without making a huge sector list the DOM or navigation model.
2. Borrow Focus's compact top-level summary metrics and collapse the inspector into a drawer/below-map panel at narrower widths. Do not use Focus's horizontal sector ribbon as the primary navigator: large snapshots make it an unbounded, low-information strip and hide volume ancestry.
3. Borrow Command's typed selector and keyboard shortcuts as accelerators. Do not make Command's map-first workspace the default: it is efficient for experts but hides coverage, revision, volume ancestry, and nearby-sector state that are essential when explaining corruption and incomplete evidence.

The selected interaction contract is revision-pinned. A navigation or enrichment response may advance only through an explicit returned revision token; stale selections remain viewable against their original revision and never silently mix facts from another revision. Every details request advertises detail support, availability, coverage, and canonical diagnostics independently.

### Follow-up full-volume mosaic decision

On 2026-08-20 the user replaced the single-sector center workspace with a full-volume requirement: multiple sectors must be visible together, every sector must retain all 64 pages as small squares, and allocation meaning such as `unreserved` must be conveyed by color.

Three follow-up variants are captured on branch `prototype/full-volume-mosaic` at commit `1205cdb` in `prototype/full-volume-mosaic.html`: responsive 8×8 sector cards, dense 64-cell sector rows, and a zoomed-out atlas field. The accepted production direction is the responsive sector-card mosaic. It preserves sector shape, shows many sectors and hundreds of pages in one viewport, and keeps page hit targets usable longer than the atlas. Dense rows remain appropriate for deterministic HTML export but are not the primary live explorer.

The live viewer exposes the complete selected volume through bounded, revision-bound continuation. It fetches 24 sector cards initially and progressively appends bounded windows while the user scrolls; the API caps one response at 64 sectors. This preserves the full-volume navigation space without constructing one unbounded response. Each sector card contains exactly 64 page cells in physical order. Allocation class supplies the base color (`system-metadata`, `unreserved`, `reserved-unallocated`, or `allocated`), while findings remain an independent outline and accessible text label.

### Follow-up replacement drill-down decision

On 2026-08-20 the user selected replacement navigation rather than a modal: selecting a sector replaces the volume mosaic with an enlarged 8×8 view of all 64 pages; selecting a page then replaces the sector with a page workspace. Explicit Back and breadcrumb controls restore the preceding level.

Three replacement variants are captured on branch `prototype/drilldown-workspace` at commit `f63a99b` in `prototype/drilldown-workspace.html`: a breadcrumb stage, a persistent context rail, and a vertically stacked technical sheet. The accepted production direction is the breadcrumb stage. It gives page cells enough room for identity/type labels, makes the hierarchy explicit, and gives page detail a two-column structure without duplicating the volume map beside it.

Page selection is an explicit deep-inspection target. When supported detail has not yet been requested, the browser automatically submits one bounded enrichment and advances to the returned immutable revision before showing structural detail. A slotted page exposes its safe slot directory independently of specialized page metadata: every validated slot row shows slot ID, record type, byte offset, and byte size, alongside a proportional structural extent canvas. Source bytes and payload remain withheld.

## Answer

The production web explorer uses a responsive Atlas shell:

- a persistent header identifies the snapshot fingerprint prefix, immutable revision, selected volume/sector, outcome, and authentication/session state;
- a left rail contains an expandable snapshot/volume tree; bounded sector continuation drives the center mosaic rather than duplicating sectors in a second navigation list;
- the center workspace renders the selected volume as a progressively loaded mosaic of keyboard-focusable sector cards, each previewing exactly 64 page cells; selecting a card exposes its 64 individually keyboard-focusable pages, with allocation, physical type, ownership conflict, encryption, support level, and diagnostic state conveyed by accessible labels as well as color;
- selecting a sector replaces the mosaic with a large 64-page grid, and selecting a page replaces that grid with page identity, support/availability/coverage, safe physical extents, typed slots, structural relationships, OOS/overflow links, and contained diagnostics; Back and breadcrumbs restore the previous level; and
- a typed selector/command palette jumps to snapshot-scoped volume, sector, page, file, slot, and OOS-value identities without accepting paths, offsets, or arbitrary query expressions.

Normal, sparse, fragmented, corrupt, and OOS-heavy snapshots use the same hierarchy. Sparse and fragmented sectors emphasize allocation runs and ownership disagreements without collapsing missing pages. Corrupt content retains validated sibling facts, marks the exact containment boundary, and keeps coverage visible. OOS-heavy pages render chains lazily with validated-prefix, terminal-condition, total-length, and traversal-budget facts; selecting a chain node navigates through typed identities rather than URLs containing secrets or byte offsets. Loading and enrichment states keep the prior immutable revision visible, reserve layout space, announce progress accessibly, and offer cancellation.

Desktop uses three panes. Medium widths keep the tree and map while moving details below the map or into a drawer. Small widths present tree, sector map, and details as ordered regions; each large region is independently scrollable, the selected page stays identifiable, and no semantic information depends on hover. The 64-page grid keeps logical reading order, arrow-key roving focus, Enter/Space selection, visible focus, sufficient contrast, non-color status labels, and screen-reader summaries. Command search is an accelerator, never the only route.

The browser receives normalized model projections only. Page byte maps are labeled structural extents—not byte dumps—and evidence locators identify entity, field, validation, and source range while reporting `bytes_withheld`. No HTML, JavaScript, API route, export, diagnostic, tooltip, accessibility label, or log contains raw bytes, hexadecimal payload, application values, ciphertext, keys, tokens, source paths, or unredacted volume strings. The artifact resolves the original prototype question; production code will reimplement the accepted hierarchy rather than merge the throwaway branch.
