Type: prototype
Status: resolved
Blocked by: 04, 06, 13

# Prototype the TUI navigation and inspection flow

## Question

How should the terminal interface express the same canonical model and drill-down as the web viewer within realistic terminal constraints? Build a cheap interactive prototype for volume and sector navigation, the 64-page sector grid, page and slot details, OOS chain links, filters, search, legends, anomaly diagnostics, resizing, mouse support, and complete keyboard operation. Decide what fidelity is shared with the web UI and what is intentionally terminal-specific; link the prototype asset from the resolution.

## Comments

### Standing human disposition

On 2026-08-19 the user directed every remaining ticket to accept the source-backed recommended option and continue without further HITL. This prototype therefore compares three terminal-native layouts against the accepted model, CLI, decoder, and disclosure contracts and records the strongest recommendation as accepted.

### Prototype artifact and validation

The throwaway implementation lives on branch `prototype/tui-flow` at commit `f8a9ca7f4305342e75c0e204c8bb5a3faf978abe`. Open the [interactive TUI-flow prototype](/home/vimkim/temp/volmap-tui-prototype/prototype/tui-flow.html) from its isolated worktree; its adjacent README records launch instructions and controls. It is a browser-hosted interaction mock used to compare terminal layouts, not production TUI code, and the branch is intentionally not merged into `main`.

One route exposes three materially different flows through `?variant=A|B|C`:

- **A — Navigator:** a persistent volume/sector tree, 64-page map, and page inspector in three side-by-side panes;
- **B — Stack:** a breadcrumb and full-width page map above selected-page and relationship/finding panes; and
- **C — Command:** a typed selector above a map-first view and compact lower details.

All variants exercise normal, sparse, fragmented, corrupt, and OOS-heavy scenarios, a loading/cancellation state, mouse selection, complete arrow-key movement over the 8×8 page grid, typed navigation, tabs, legends, relationship nodes, and canonical diagnostics. The terminal control simulates 160×45, 120×36, and 80×24. JavaScript syntax, HTTP delivery, mouse controls, and headless Chrome renders were validated for all three at a 160-column presentation and for Stack at 80×24. The artifact contains no raw/hex action and no payload-valued sample.

### Accepted prototype verdict

Under the standing disposition, the accepted production direction is **Stack as the adaptive TUI shell**, with selective affordances from the other variants:

1. Stack gives the 64-page sector—the operator's main spatial unit—the largest stable region while keeping revision, coverage, selection, relationships, and diagnostics visible. It degrades at 80×24 by showing the map plus one tabbed detail region, rather than squeezing three unreadable panes.
2. Borrow Navigator's volume/sector tree as a toggleable overlay or wide-terminal pane, not a permanently reserved narrow-screen column. This retains nearby-sector orientation without making hierarchy traversal the only entry path.
3. Borrow Command's typed selector and `/` accelerator. Do not make it the default screen because command recall cannot replace discoverable coverage, legends, and anomaly context.

The TUI is a projection of the same immutable revision and canonical entities as CLI, web, and HTML. It neither reparses volumes nor invents terminal-only facts. Mouse support invokes the same focus/selection/tab actions as keyboard input and is optional; every action remains reachable without it.

## Answer

The production TUI uses a responsive stacked layout. Its title row shows snapshot fingerprint prefix, immutable revision, outcome, active scenario/filter summary, and selected typed identity. The main region renders one 64-page sector as an 8×8 grid in logical page order. The lower region uses tabs for structure, slots, chain/relationships, findings, and coverage. On wide terminals, an optional left hierarchy pane and a simultaneous right relationship pane may open; at 120 columns the hierarchy becomes an overlay; at 80×24 only the map and active lower tab are rendered. Resize recomputes layout without changing revision, selection, active filters, or focus target.

The TUI shares the web viewer's facts and terminology exactly: snapshot/volume/sector/page/file/slot/OOS identities, page detail support, availability, coverage, diagnostics, allocation and ownership claims, structural extents, and validated relationships. It shares stable legends and status labels, but not pixel geometry, pointer-dependent disclosure, or web navigation history. Terminal-specific rendering uses text abbreviations, Unicode only when terminal capability permits, an ASCII fallback, column-aware truncation, scroll indicators, and a persistent help/status row. Color is supplemental; every page cell and finding has a textual or symbolic state.

Keyboard operation is complete and modal state is shallow:

- `Tab`/`Shift-Tab` cycles visible regions; arrow keys use roving focus inside the 8×8 grid and lists; `Enter` selects or follows a typed relationship; `Esc` closes overlays or cancels an active enrichment;
- `/` opens typed-selector search; `g` opens a jump prompt; `[`/`]` move to the previous/next sector; `n`/`N` move between findings; `f` opens filters; `?` opens context help; and `q` requests exit without affecting the snapshot;
- number keys or mnemonic tab keys select Structure, Slots, Chain, Findings, and Coverage; and
- commands that may start enrichment display their scope and revision before execution, return focus to the initiating entity, and never silently advance to a newer revision.

Mouse clicks select the same cells, tabs, tree nodes, relationship nodes, and scroll regions. Wheel events scroll only the hovered pane. Mouse absence, unsupported terminal mouse reporting, or screen-reader operation loses no command. Focus is always visible, the current region is named, and terminal resize announces the new layout once without flooding status output.

Filters operate on normalized fields only: volume/file identity, allocation state, recognized physical type, detail support, encryption state, diagnostic code/severity, and coverage state. Search accepts the typed selectors already defined by the CLI contract, not regex over payloads or arbitrary byte offsets. A match list states revision and result count and preserves the previous context until the operator selects a result.

Normal, sparse, fragmented, corrupt, and OOS-heavy snapshots keep the same layout. Sparse and fragmented cases emphasize allocation runs and ownership conflicts. Corrupt cases retain valid cells, place findings at the exact containment boundary, and keep the coverage tab visible. OOS chains are lazy typed-link lists with validated-prefix, length, terminal, and budget facts; following a node updates selection but never displays fragments. Loading retains the prior revision, reserves the destination pane, shows bounded progress when knowable, and exposes `Esc` cancellation.

The disclosure boundary is identical across adapters. Structural byte maps use labeled extents and evidence locators ending in `bytes withheld`; the TUI never offers raw bytes, hexadecimal payloads, application values, ciphertext, secrets, unredacted paths, or token material. Copy mode exports only the same normalized, redacted human/JSON facts available through explicit CLI commands. The production implementation should reproduce this interaction contract in the chosen Rust terminal library rather than merge the throwaway browser mock.
