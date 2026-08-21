Label: wayfinder:grilling
Type: grilling
Status: resolved
Assignee: codex
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Prototype terminal interaction parity across Volume, Sector, and Page](01-prototype-terminal-interaction-parity.md)

# Define semantic color, glyph, and fallback mappings

## Question

How should every web semantic used by the three parity views map to terminal presentation without collapsing independent dimensions? Define allocation classes, known occupied/free percentage, unknown occupancy, physical page type, findings, focus, selection, slotted header, live record extents, fragmented and contiguous free regions, slot directory, and allocated/empty/deleted slots across ANSI color, Unicode block-glyph, monochrome, and ASCII modes. Resolve precedence, legends, textual labels, Unicode display-column measurement, terminal-control sanitization for source-derived labels such as class/table names, contrast, and what information may compact—but never disappear—at 80×24 and 60×20.

## Answer

### Decision

The user accepted every recommendation over two semantic decision rounds,
explicitly retained the already accepted Crossterm architecture after a Ratatui
reconsideration, and confirmed the complete shared understanding. Production
uses one closed, private semantic encoder inside the accepted
`AtlasRenderer`. It composes every storage and interaction dimension into a
stable channel; it does not publish a theme API, a presentation catalog, or a
widget protocol.

The visual direction is **layered microcards** at the wide and stacked tiers,
with one fixed seven-column Page strip at the compact tier. The strip is also
the canonical abbreviated form used wherever a Sector card needs a dense 8×8
preview:

```text
[focus][physical type × 2][allocation][occupancy][finding][selection]
```

Every channel retains its position when inactive. A blank channel means the
corresponding state is absent; it never causes neighboring meanings to shift.
The renderer may add full labels, a proportional meter, borders, and color
when space permits, but it never removes or merges these channels.

This design was selected over two rejected alternatives. A flexible private
presentation catalog would make palettes, profiles, and templates extensible,
but Atlas has only three fixed screens and four required presentation
profiles, so the catalog adds a second rendering language without a caller.
Separate synchronized allocation/type and occupancy/finding planes make the
dimensions obvious but force the operator to correlate two grids. The chosen
composition keeps one physical Page in one visual location while preventing
one fact from replacing another.

### Presentation profiles and invariants

A terminal presentation profile is the Cartesian product of two independent
capability axes:

```text
Color: ANSI | Monochrome
Glyphs: Unicode | ASCII
```

The four supported profiles are therefore ANSI/Unicode,
monochrome/Unicode, ANSI/ASCII, and monochrome/ASCII. The terminal host
resolves a profile and supplies it through the existing `RenderSurface`;
profile detection and terminal I/O remain outside the semantic encoder.
Unicode with deterministic narrow-width measurement is the default glyph
policy, and an explicit ASCII profile is the compatibility escape hatch. The
renderer does not guess from locale.

Changing profile may change glyphs and style only. It must not change:

- the Inspection revision or any projected fact;
- the complete Volume/Sector/Page topology or ordering;
- enabled actions, focus topology, hit regions, or scroll regions;
- whether a Page, finding, byte range, Slot entry, class/table attribution, or
  legend meaning is available; or
- the semantic identity attached to a rendered control.

The semantic encoder consumes only typed `AtlasScene` content. It never parses
display labels, diagnostic messages, selectors, paths, HTTP values, or raw
volume bytes to choose a presentation state. The accepted
`AtlasRenderer::compose`/`PreparedFrame::present` interface and repository-owned
cell frame remain unchanged. Ratatui is not added: at the accepted baseline its
minimal Crossterm integration violates the repository's categorical
duplicate-version policy, expands Crossterm features, and still does not own
Atlas navigation, semantic hit regions, revision discipline, or text safety.

### Page-cell vocabulary

Interaction markers deliberately remain single-column ASCII in every glyph
profile so focus and selection do not depend on ambiguous symbol width:

| Channel | Present | Absent | Meaning |
| --- | --- | --- | --- |
| Focus | `>` | space | Prospective roving focus |
| Finding | `!` | space | One or more typed Diagnostic occurrences affect the Page |
| Selection | `*` | space | Committed selection in the Atlas trail |

The marker is only a summary. The focused descriptor and Findings region keep
the occurrence count, stable code, severity, affected Entity references, and
evidence locus. Multiple findings never become multiple Page cells, and the
renderer never infers severity from a message or subject string.

Allocation uses one stable token in every profile:

| Allocation class | Token | Full label |
| --- | --- | --- |
| `system-metadata` | `S` | System metadata |
| `allocated` | `A` | Allocated |
| `reserved-unallocated` | `R` | Reserved, unallocated |
| `unreserved` | `U` | Unreserved |

Physical type uses an exhaustive two-column code. A known physical
`PageType::Unknown` is distinct from a type that was not inspected or is not
supported:

| Projection value | Code | Full label |
| --- | --- | --- |
| known `unknown` | `UN` | Unknown physical type |
| known `file-table` | `FT` | File table |
| known `heap` | `HP` | Heap |
| known `volume-header` | `VH` | Volume header |
| known `volume-bitmap` | `VB` | Volume bitmap |
| known `query-result` | `QR` | Query result |
| known `extensible-hash` | `EH` | Extensible hash |
| known `overflow` | `OF` | Overflow |
| known `oos` | `OS` | OOS |
| known `area` | `AR` | Area |
| known `catalog` | `CA` | Catalog |
| known `btree` | `BT` | B-tree |
| known `log` | `LG` | Log |
| known `dropped-files` | `DF` | Dropped files |
| known `vacuum-data` | `VD` | Vacuum data |
| projection `unknown` | `??` | Physical type not inspected |
| projection `unsupported` | `--` | Physical type unavailable |

The type mapping must be exhaustive over the shared projection vocabulary; a
new physical type cannot silently inherit `UN`, `??`, or another type's code.

Occupancy encodes the occupied portion; the adjacent full descriptor always
states both exact values as `occupied P% / free F%`:

| Occupancy | Unicode compact | ASCII compact | Full meter |
| --- | --- | --- | --- |
| Known 0% | `0` | `0` | Eight free cells |
| Known 1–100% | `▁` through `█` | `1` through `8` | Eight occupied/free cells |
| Unknown | `?` | `?` | Eight `?` cells and `occupancy unknown` |
| Explicitly not applicable | `-` | `-` | `occupancy not applicable` |

For a positive known percentage, the visual bucket is
`ceil(occupied_percent × 8 / 100)`, clamped to 1–8. Unicode meters use `█` for
occupied and `░` for free; ASCII meters use `#` and `.`. This display bucket
does not replace the exact projection value. In particular, the existing
validated 15-level occupancy facts and their exact occupied/free percentages
remain available in the focused descriptor and Page facts.

`Not applicable` is emitted only when the semantic scene explicitly supplies
that state. The renderer never manufactures it from allocation class and never
converts projection `unknown` to `-`. Known zero, unknown, and not applicable
are therefore three distinct states in every profile.

### Page byte map and Slot directory

The exhaustive 16,344-byte slotted-page distribution has five semantic region
tokens:

| Region | Token | Full label |
| --- | --- | --- |
| Slotted header | `H` | Header |
| Live record extent | `R` | Record, including Slot id and record type |
| Fragmented free interval | `F` | Fragmented free |
| Contiguous free interval | `C` | Contiguous free |
| Slot directory | `D` | Slot directory |

At Page detail, the exact ordered rows with half-open offset/length ranges are
authoritative. The raster is a proportional navigation aid over those rows. A
raster column that represents bytes from exactly one class uses that class's
token or pattern. If compression places two or more classes in the same
terminal column, it uses `+` with the legend `mixed column`; it does not choose
a winning class. The exact contributing rows remain visible in the same
Distribution region at wide and stacked tiers and remain scrollably reachable
in the compact Distribution tab.

The complete physical Slot directory remains ordered by Slot id and uses:

| Slot state | Token | Full label |
| --- | --- | --- |
| Allocated | `A` | Allocated |
| Unallocated/empty | `E` | Empty (unallocated) |
| Deleted | `D` | Deleted |

Every Slot row also retains its exact entry offset, length, structural record
type, and any live-record extent. The two tombstone record types remain full
text and are not collapsed into a generic empty record. Empty and deleted
entries have no live extent. Region tokens and Slot tokens are contextual and
never substitute for the shared typed geometry.

### Composition and precedence

There is no destructive semantic precedence. The encoder assigns each fact or
interaction state its own cells and style role:

- A finding never replaces allocation, unlike the legacy TUI's `!` marker.
- Focus never replaces committed selection, and selection never hides focus.
- Allocation never replaces physical type or occupancy.
- Unknown occupancy never borrows the visual for zero or free space.
- A filtered/dimmed item retains every token and remains focusable and
  descendable under the accepted navigation contract.
- Source invalidation, enrichment progress, errors, and notices use their
  accepted scene regions; they do not overwrite canonical Page facts.
- Overlay geometry may shadow underlying hit regions under ticket 04's layout
  precedence, but closing it reveals an unchanged semantic scene.

ANSI style is local to the cells owned by a channel. There is no whole-card
foreground style that can turn six independent meanings into one color. A
mixed raster column is the sole visual aggregation, is explicitly marked `+`,
and is always backed by exact rows.

### ANSI roles, monochrome, and contrast

ANSI mode requests named foreground/attribute roles over the operator's
default terminal background:

| Semantic role | ANSI reinforcement | Monochrome reinforcement |
| --- | --- | --- |
| System metadata | Magenta | `S` |
| Allocated | Green | `A` |
| Reserved, unallocated | Blue | `R` |
| Unreserved | Default foreground | `U` |
| Occupancy | Cyan | Meter/token |
| Unknown occupancy | Yellow | `?` pattern |
| Finding | Red | `!` |
| Focus | Yellow plus reverse on the focus cell | `>` plus reverse |
| Selection | Cyan plus underline on the selection cell | `*` plus underline |
| Header | Magenta | `H` |
| Live record | Green | `R` |
| Fragmented free | Yellow | `F` |
| Contiguous free | Cyan | `C` |
| Slot directory | Blue | `D` |
| Allocated Slot | Green | `A` |
| Empty Slot | Default foreground | `E` |
| Deleted Slot | Red | `D` |

Color and terminal attributes are reinforcement, never the normative carrier.
User-configured ANSI palettes cannot support a universal numeric contrast
claim, so Volmap does not copy the browser's fixed RGB values or claim WCAG
ratios for an arbitrary terminal palette. Controlled reference captures must
meet at least 4.5:1 for normal text and 3:1 for non-text interaction indicators
against their authored background. The monochrome profiles, stable tokens,
fixed channel positions, borders, meters, and full labels are the compatibility
contract when palette contrast is unknown.

### Legends and tier compaction

Every screen provides a plain-language descriptor for the focused semantic
item. A Page descriptor expands allocation, physical type, exact occupancy,
finding count, focus/selection relationship, and file/class/table attribution;
byte and Slot focus expands the exact range or entry. Inline truncation is
permitted only when the complete sanitized value is reachable in the focused
descriptor or the corresponding scrollable Facts/Distribution/Slots region.

Legend policy follows the accepted tiers:

- At 120×36 and larger, show the complete inline legend for every encoding in
  the current screen and Page regions.
- At 80×24, wrap or stack the complete contextual legend without deleting an
  entry.
- At 60×20, show one concise legend for the active screen or Page region and
  keep the complete screen-specific legend in contextual Help.
- In the reversible too-small frame, retain state and make the complete legend
  available again after recovery; the too-small scene is not a parity view.

Tier changes may reduce simultaneous cards, meter length, labels beside a
token, or simultaneous Page regions. They may never sample or omit sectors,
remove one of a Sector's 64 Pages, remove a Page-cell channel, merge known zero
with unknown, omit an exact byte/Slot row from its scrollable region, or change
an action. Wide Page facts/distribution, stacked Page content, and compact Page
tabs remain the layouts accepted by tickets 01, 03, and 04.

### Terminal text safety and display columns

Every source-derived label—including file role, class/table name, diagnostic
text, selector echo, and attribution—passes through the single private
`TerminalText` path before display-column measurement, wrapping, clipping,
padding, or cell placement. Styling remains separate cell metadata; sanitized
text can never manufacture ANSI.

Sanitization is deterministic and visibly reversible:

- TAB, LF, CR, and ESC become `[TAB]`, `[LF]`, `[CR]`, and `[ESC]`.
- Every other C0/C1 control, DEL, and Unicode bidirectional-format control
  becomes uppercase `[U+XXXX]` (using more than four hexadecimal digits when
  needed).
- A zero-column scalar or cluster without a printable base becomes the same
  `[U+XXXX]` form for each constituent scalar. Combining marks, variation
  selectors, and joiners may remain only inside a safe visible grapheme
  cluster; they never stand alone or carry bidirectional control.
- An empty source value becomes `[empty]` where an otherwise blank field would
  be ambiguous.
- Unicode mode retains safe source grapheme clusters exactly and performs no
  normalization.
- ASCII mode first substitutes the ASCII presentation vocabulary, then renders
  every non-ASCII source scalar as `[U+XXXX]`.

After sanitization, segmentation uses a direct exact dependency on
`unicode-segmentation` and display measurement uses a direct exact dependency
on `unicode-width`. Implementation should initially pin the audited releases
`unicode-segmentation = "=1.13.3"` and `unicode-width = "=0.2.2"`, subject to
the repository's normal locked dependency gates at implementation time. Use
`unicode-width`'s normal width, not its CJK-width operation: East Asian
Ambiguous characters are deterministically one column. The ASCII profile is
the explicit fallback for terminals whose rendering disagrees; locale does not
silently change the same profile's frame.

Clipping, wrapping, and ellipsis occur only at sanitized grapheme boundaries.
A wide grapheme owns its leading cell and explicit continuation cells; erasing,
overpainting, diffing, or clipping it clears every continuation cell so stale
halves cannot corrupt neighbors. A render fault, rather than unchecked output,
results if a trusted presentation glyph violates its declared cell width.

### Prototype evidence

The throwaway comparison is isolated on branch
`prototype/tui-semantic-rendering` at commit
`c6ccc35cf7d4b45c6c79ec562912fbc358ef7517`. Open the
[interactive semantic prototype](/home/vimkim/temp/volmap-tui-semantic-prototype/prototype/tui-semantic-rendering.html)
or its adjacent README. It compares layered microcards, fixed tuples, and dual
planes on one route and independently switches Volume/Sector/Page, 120×36,
80×24, 60×20, ANSI/monochrome, and Unicode/ASCII.

The first compact layered variant exposed that physical type had disappeared;
the corrected artifact adds the fixed seven-column strip and a dedicated
60×20 monochrome/ASCII capture. This failure is why the strip and exhaustive
profile/tier matrix are normative rather than illustrative. The prototype is
not production Rust code and does not enter `main`.

### Compatibility gates

Exercise behavior through the accepted `AtlasRenderer` seam and shared typed
Projection fixtures, not through private color or glyph helper tests alone:

- Golden the same representative Volume, complete 64-Page Sector, and Page
  scenes at 120×36, 80×24, and 60×20 under all four presentation profiles.
  Assert identical semantic controls, identities, focus topology, hit regions,
  scroll regions, and reachable facts across profiles.
- Exhaustively cover the four allocation classes; known 0 and representative
  positive occupancy buckets through 100; unknown and explicit not-applicable
  occupancy; every physical type plus projection unknown/unsupported; and all
  combinations of finding, focus, and selection. Assert the seven channel
  columns never shift or overwrite one another.
- Assert that focused descriptors preserve exact occupied/free percentages
  after visual re-bucketing, including the existing 7/93 fixture. Unknown must
  never render as `0`, an empty known meter, or not applicable.
- Make the physical-type mapping exhaustive at compile time or through a
  completeness test so a new shared type cannot silently reuse an existing
  code.
- Retain the shared golden slotted distribution: 16,344-byte conservation,
  header, ordered live extents, every fragmented and contiguous free interval,
  complete directory, and every Slot entry. Exercise raster columns containing
  one region and multiple regions and prove `+` never removes an exact row.
- Cover allocated, unallocated/empty, and both deleted tombstone Slot forms;
  preserve Slot id, entry range, record type, and absence of a live extent for
  empty/deleted entries.
- Prove finding, focus, selection, filtering, progress, and invalidation
  compositing cannot mutate allocation/type/occupancy tokens or their full
  labels. Pair keyboard and mouse traces to the same semantic controls.
- Fuzz and golden-test `TerminalText` with every C0/C1 control, ESC, DEL,
  embedded line endings, bidi controls, leading combining marks, isolated
  joiners/selectors, valid combining and ZWJ graphemes, East Asian wide and
  ambiguous text, emoji, long labels, and empty strings. No output may contain
  an unsanitized control, split a grapheme, leave a stale continuation cell,
  overrun a surface, or alter geometry.
- Include hostile file/class/table attribution fixtures at the `cba72cd`
  compatibility boundary. Sanitization and truncation may change only the
  terminal spelling; the complete safe label remains reachable and never
  becomes navigation identity.
- Test complete, wrapped, contextual, and Help legends at their respective
  tiers. At 60×20, prove every compact token can be expanded without leaving
  the current revision or navigation trail; exercise 59×19 suspension and
  recovery.
- Check the controlled ANSI reference palette at 4.5:1 text and 3:1 non-text
  contrast, then prove monochrome equivalence structurally instead of relying
  on screenshot color comparison.
- Add the two direct exact Unicode dependencies only after `cargo test --locked`,
  `cargo clippy --locked`, `cargo deny`, deterministic notices/SBOM, static
  x86-64 musl, and two-checkout reproducibility gates pass. Do not add Ratatui
  or relax duplicate-version policy as part of implementation.
- Keep existing web allocation/occupancy mosaics, complete distribution,
  Slot-state presentation, Page/Sector attribution, immutable revision/history,
  and browser behavior unchanged. Terminal token or palette decisions never
  enter shared facts or web formatting.

No production TUI is implemented by this resolution. No new ticket is created:
numeric viewport, overscan, frame-memory, and latency ceilings remain owned by
[ticket 07](07-set-viewport-resource-budgets.md), and complete cross-adapter
acceptance remains owned by [ticket 08](08-define-parity-verification-contract.md).
The stable cross-ticket term `Terminal presentation profile` is recorded in
[`CONTEXT.md`](../../../CONTEXT.md); `SemanticEncoding`, token tables, glyph
buckets, and ANSI roles remain private implementation vocabulary.
