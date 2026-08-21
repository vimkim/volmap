Label: wayfinder:grilling
Type: grilling
Status: resolved
Assignee: codex
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Define the shared projection boundary for terminal parity](02-define-shared-projection-boundary.md), [Define the TUI navigation, focus, and history state model](03-define-navigation-focus-history-model.md), [Choose the terminal rendering architecture and dependency boundary](04-choose-rendering-architecture.md), [Define automatic enrichment and immutable-revision transitions](05-define-enrichment-revision-lifecycle.md), [Define semantic color, glyph, and fallback mappings](06-define-semantic-terminal-rendering.md), [Set volume viewport and rendering resource budgets](07-set-viewport-resource-budgets.md)

# Define the TUI parity verification contract

## Question

What evidence must prove the redesigned TUI satisfies terminal interaction parity before implementation is accepted? Define shared-projection parity tests, navigation and immutable-revision state tests, deterministic PTY or terminal-buffer captures for 120×36, 80×24, and 60×20, ANSI/Unicode and monochrome/ASCII cases, keyboard/mouse equivalence, resize and cancellation races, large-volume responsiveness, fragmented slot distributions, findings and coverage, non-disclosure, web-regression gates, and the boundary between stable semantic assertions and brittle pixel snapshots.

## Working agreement

Resolve this ticket on branch `wayfinder/tui-parity-verification` in the sibling
worktree `/home/vimkim/temp/volmap-tui-parity-verification`. Keep `main`
unchanged while the decision is being developed. After the user confirms the
complete shared understanding and the resolution passes its verification
audit, squash-merge the branch to local `main` as one commit. Ticket 09 remains
unclaimed and unmodified during this session.

## Answer

### Decision and audited baseline

The user accepted every recommendation over three verification-policy rounds
and confirmed the complete shared understanding. Terminal interaction parity
is accepted only through an **interface-aligned evidence stack**. Tests prove
shared typed facts at the Projection workspace, deterministic state and
effects at `AtlasMachine`, semantic cells and committed geometry at
`AtlasRenderer`, terminal lifecycle at the scripted/PTY host, and unchanged
HTTP/browser behavior at the web adapter. No layer substitutes for another.

Cross-adapter parity does not mean equal JSON, terminal strings, DOM text,
screenshots, or pixels. It means that web and TUI receive the same typed facts
for one exact immutable revision and each adapter proves a complete, faithful
mapping into its own interface. Comparing before formatting preserves
legitimate adapter differences and prevents presentation text from becoming a
second semantic protocol.

The map's `cba72cd` source commit remains the minimum semantic compatibility
boundary. In addition, every test already passing at the implementation
branch's merge-base remains a regression gate, so behavior added after that
pinned map baseline cannot be silently erased. At the ticket's audited
`4bf0a74` merge-base, `cargo +1.97.1 test --locked` passed 167 tests; the one
ignored test was the manual Inspection resource benchmark. The legacy TUI had
only four private helper tests, no Atlas state or cell-buffer tests, no PTY
test, and no terminal resource benchmark. The Rust web suite exercised
handlers, JSON, and source-level asset contracts but did not execute browser
DOM/history behavior. These are evidence gaps, not a new baseline test count
that future implementations must preserve numerically.

### Evidence layers and ownership

The blocking test families are:

| Gate family | Interface under test | Normative evidence |
| --- | --- | --- |
| `PAR-PROJECTION` | Projection workspace | Exact-revision typed facts, ordering, exhaustive bounded windows, enrichment publication/cancellation, diagnostics, coverage, non-disclosure |
| `PAR-STATE` | `AtlasMachine::start`/`advance` | Initial state plus ordered semantic event traces, scenes, effects, ancestry, focus, anchors, overlays, enrichment and adoption |
| `PAR-RENDER` | `AtlasRenderer::compose` and `PreparedFrame::present` | Normalized `CellGrid`, semantic styles, `LayoutCommit`, hit/focus/scroll geometry, profiles, tiers, diff/presentation faults |
| `PAR-HOST` | Scripted terminal host and production Crossterm host | Event ordering/coalescing, real TTY lifecycle, resize, input normalization, restoration and terminal faults |
| `PAR-WEB` | Web handlers and real embedded browser viewer | Existing HTTP/JSON shape plus semantic DOM navigation, exact URLs/history, enrichment and invalidation |
| `PAR-DISCLOSURE` | Every Projection and adapter output surface | Positive structural facts and absence of forbidden payload, ciphertext, key, path and control sentinels |
| `PAR-RESOURCE` | Atlas reservoir, renderer, scheduler and presenter | Exact memory ledger, cardinality/work limits, redraw cadence and controlled latency distributions |
| `PAR-RELEASE` | One exact candidate commit and shipped static binary | Locked suite, test-infrastructure locks, static musl, reproducibility, supply chain, PTY/browser and cross-distribution behavior |

Tests cross the same deep interfaces production callers use. Private helper
tests may isolate arithmetic or faults, but they cannot be the only evidence
for a gate. Do not publish a test-only Projection recipe interface, renderer
widget seam, clock protocol, mock web schema, or alternate state machine. The
real in-process modules plus their already accepted private scripted adapters
are the test surface.

Implementation creates a checked-in parity matrix mapping each accepted
requirement from tickets 01 through 07 to a stable gate name, fixture, tested
interface, expected invariant, and blocking job. Test names cite their gate
names. A numeric line-coverage percentage is advisory only: it cannot prove
identity, ordering, races, geometry, disclosure, or completeness. One test may
cover several gates only when its assertions name and independently prove each
invariant.

### Canonical fixtures

One repository-owned parity corpus supplies the same exact-revision Projection
frames to both adapters. Adapter tests must not independently reconstruct what
the shared facts ought to mean. The corpus has named, deterministic cases for:

- multiple Volumes and Sectors containing all four allocation classes;
- known zero and positive occupancy, the established exact 7/93 case, unknown
  occupancy, every physical Page type, projection unknown/unsupported, and
  finding/focus/selection combinations;
- Page and Sector file/class/table attribution states, including resolved,
  unresolved, mixed, absent, reserved-for, and hostile source labels;
- a complete 64-Page Sector and bounded contiguous multi-Sector windows;
- the exact fragmented 16,344-byte slotted Page with header, ordered records,
  fragmented and contiguous free intervals, complete directory, and
  allocated, empty, and both tombstone Slot forms;
- complete, partial, unknown-total, diagnostic-bearing, and invalidated
  coverage/outcome states with typed affected Entity references;
- before, working, valid publication, decode-diagnostic publication,
  validated-prefix publication, cancellation, stale-base, late offer, and
  invalidation revisions; and
- unique forbidden disclosure sentinels plus positive safe structural facts.

Fixtures carrying storage facts use the existing pinned volume corpus or
deterministic builders and pass through the real Projection workspace. A
renderer-only `AtlasScene` builder is permitted for presentation states that
cannot originate from volume bytes, such as a too-small frame or an adapter
overlay; it cannot invent a storage fact to avoid a Projection test. Partial
terminal entry and injected presentation faults instead use the private host
and presenter test adapters.

Large cases remain lazy and deterministic rather than checked-in giant files:

- a 33,554,432-Sector source materializes only requested windows and supports
  first/middle/last direct jumps;
- a 257-Sector source is exhaustively traversed in both directions to prove
  exact reachability without sampling;
- the real 4,096-Sector sparse fixture retains end-to-end Projection coverage;
- maximum Page fixtures expose 4,078 Slot rows and the conservative 8,160
  Distribution rows without eagerly formatted output; and
- hostile strings cover controls, bidi formatters, combining/ZWJ sequences,
  wide and ambiguous characters, emoji, long attribution, and empty labels.

Every fixture pins snapshot, revision, Entity identities, ordering, expected
availability/coverage dimensions, and any allowed explicit-target value. A
failure must name the fixture and exact revision so a mixed-frame defect cannot
look like a formatting mismatch.

### Stable assertions and golden boundary

The following are stable product assertions:

- typed Projection values, identity, ordering, exact revision, availability,
  coverage and diagnostic occurrence references;
- `AtlasMachine` state, ordered effects, trail, focus, selection, semantic
  anchors, overlays, request/offer identity and adoption result;
- normalized repository cell symbols, semantic style roles, clipping and
  continuation cells at an accepted extent/profile;
- `LayoutCommit` tier, generation, semantic controls, focus graph, hit regions,
  precedence and scroll extents; and
- presence and reachability of every required fact, action, legend, region and
  fallback distinction.

The following are deliberately non-normative and cannot fail parity merely by
changing: raw ANSI write chunking, syscall count, browser pixels, screenshots,
font rasterization, operator palette RGB values, DOM class names, CSS pixel
coordinates, cache-hit order, absolute paths, timestamps, process ids, heap
addresses, allocator overhead, and uncontrolled terminal latency. Tests may
inspect raw terminal output for safety and required entry/restoration commands,
but do not golden its incidental byte grouping.

The normative checked-in renderer matrix contains the same representative
Volume, Sector, and Page scenes at 120x36, 80x24, and 60x20 under all four
terminal presentation profiles. That is **36 `CellGrid`/`LayoutCommit` golden
pairs**. Each pair is one human-readable deterministic artifact containing a
header with fixture/revision/extent/profile, the exact normalized cells and
semantic style roles, then the semantic commit sidecar. The combined artifact
prevents a frame and its geometry from being updated independently.

Focused additional goldens cover selector/filter and Help/About overlays,
trusted and unknown-total progress, findings, diagnostic/invalidated state,
and 59x19 reversible suspension. These cases are not multiplied into another
full Cartesian snapshot set. Structural tests exercise their semantics at
every tier/profile, while exhaustive token combinations, hostile text, and
large Page rows use table/property assertions instead of thousands of brittle
goldens. The 256x128 cap and adversarial `u16` extents are structural resource
tests rather than oversized snapshots.

Golden generation writes candidates beneath `target/parity-goldens/`; it
never overwrites checked-in files. Updating a golden requires reviewing the
cell and semantic-commit diff, stating the accepted behavior that authorizes
it, and manually replacing the artifact. There is no in-place `BLESS`, update
environment variable, or retry path that can make an unexpected frame pass.
Screenshots, recordings, and prototype captures remain optional review aids.

### Projection and cross-adapter gates

`PAR-PROJECTION` retains and moves existing semantic oracles to the shared
interface where appropriate:

- exact 64-Page Sector order, the four allocation classes, known/unknown
  occupancy including 7/93, and all Page/Sector attribution states;
- contiguous bounded Sector windows which are exhaustive when followed and
  never sample or duplicate an identity;
- one atomic Page result containing facts, deep-detail disposition, complete
  safe Slot directory, and exhaustive byte map from the same revision;
- exact 16,344-byte conservation, ordered live extents, every free interval,
  directory placement and every Slot state;
- diagnostics with stable occurrence identity and typed affected Entity
  references, plus complete/partial/unknown coverage ledgers and outcome
  precedence;
- every enrichment eligibility disposition and final publication protocol,
  including Page cancel/no publication, linked-prefix cancel/publication,
  decode-diagnostic publication, source invalidation precedence, stale head,
  one successor revision, and idempotence; and
- immutable old revisions and no raw bytes, ciphertext, key material, source
  path, web cursor, selector, terminal text, or adapter navigation state at the
  seam.

The same corpus frame feeds TUI and web mapping tests. Cross-adapter assertions
compare typed identities and facts before formatting. TUI then proves that
each fact has the ticket 06 token/label/region and remains reachable; web proves
its existing JSON/DOM representation. Neither adapter parses the other's
output, and TUI test serialization is never made part of JSON schema version
1.

### AtlasMachine traces and properties

`PAR-STATE` replays named deterministic traces through
`AtlasMachine::start`/`advance` for:

- Volume -> Sector -> Page descent, repeated/breadcrumb ascent, exact focus and
  independent scroll restoration, and selector/finding canonical trails;
- every retained accelerator: arrows, Enter, Esc, Backspace, `[`, `]`,
  PageUp/PageDown, `/`, `g`, `f`, `n`, `N`, `1` through `6`, `d`, Tab,
  Shift-Tab, `j`, `k`, `?`, and `q` under editor/overlay precedence;
- the fixed Sector 8x8 rover, renderer-supplied Volume focus graphs, first/last
  edges, nonmatching filters, invalid selectors/filters, and misleading
  diagnostic messages/subjects that must never drive navigation;
- every Page region and independent semantic anchor across scrolling,
  region/tier changes, descent/ascent, revision adoption, and temporary 59x19
  suspension;
- every automatic enrichment eligibility state, exactly one attempt per visit
  and base, no redraw-triggered work, trusted progress, cancellation,
  draining/replaceable intent, explicit retry, exact offers and whole-trail
  adoption; and
- simultaneous input/completion, cancel/publication, navigation/late-result,
  wrong request/visit/target/snapshot/base, adoption rollback, invalidation and
  quit/fault cleanup races.

Keyboard and mouse adapters are paired by starting from identical state and
feeding the semantic action produced by each input path. Every committed hit
region must have a keyboard path; clicking an Entity equals focusing then
activating it, and wheel routing equals the corresponding active-region
scroll. The final Atlas state and ordered effects must be identical. Physical
key bytes and mouse escape sequences need not be equal.

Seeded property traces generate arbitrary valid/invalid event and worker-signal
sequences and assert one snapshot/revision per scene, valid ancestry, unique
focus, at most one overlay/request/offer, bounded anchors, deterministic
effects, input-first race precedence, no adoption without an exact active match
or explicit offer, and stale-generation safety. A failure prints its seed and
a deterministic reduced event sequence accepted directly by the scenario
replayer. Use repository-owned deterministic generation/reduction by default;
do not add a Rust property framework merely to wrap these closed events.

### Renderer, geometry, and semantic profiles

`PAR-RENDER` uses the 36 core goldens plus structural/property tests to prove:

- wide, stacked and compact layouts retain the accepted regions and actions;
- ANSI/Unicode, monochrome/Unicode, ANSI/ASCII and monochrome/ASCII preserve
  identities, controls, focus topology, scroll regions and facts;
- all allocation, type, occupancy, finding, focus and selection channels remain
  independent, with known zero distinct from unknown and exact percentages in
  the focused descriptor;
- byte raster aggregation never removes exact Distribution rows, every Slot
  remains reachable, and virtualization prepares no more than 384 rows;
- every source label passes through `TerminalText`; no unsafe control survives,
  no grapheme is split, wide-cell continuation is consistent, and complete
  sanitized text remains reachable;
- every painted control produces its own clipped semantic hit region,
  equal-precedence regions do not overlap, focus edges name valid controls,
  overlay precedence is exact, and no input code reconstructs coordinates;
- composition is deterministic from scene/surface/version, includes one exact
  revision, and remains inside 32,768 cells and the accepted card/window bounds;
  and
- injected compose, short-write and flush faults publish no invalid commit and
  never advance the prior successful frame cache.

Tests inspect the repository cell grid and semantic styles, not terminal RGB
pixels. The controlled ANSI palette contrast check remains numeric, while
monochrome/ASCII equivalence is structural. A presentation fallback may change
symbols and styles only; a missing fact, control or semantic region is a
failure.

### Scripted host and real PTY boundary

The private scripted host is the ordinary deterministic event-loop adapter. It
supplies exact terminal extents, profiles, monotonic ticks, ordered input,
worker results, partial writes and flush faults, and records prepared frames
and commits. Ordinary locked tests use it for resize storms, 60/10 Hz
coalescing, capacity-one progress, 32-event input draining, input-first
ordering, no idle redraw, stale signals and every state/layout combination.
They never sleep or depend on a real terminal clock.

A controlled Linux job uses a checked-in, exactly versioned Expect harness and
real pseudo-terminals. It covers only behavior that a scripted host cannot
prove:

- non-TTY rejection before terminal mutation;
- successful and partially failed entry into raw mode, alternate screen, mouse
  capture and hidden cursor;
- key, mouse and resize delivery through Crossterm normalization;
- normal quit, cancellation during quit, every reachable typed terminal error,
  write/flush failure and cursor/mouse/screen/raw-mode restoration; and
- best-effort panic-hook cleanup in a private child-test subprocess.

Normal public interaction paths run against the exact candidate static-musl
`volmap` binary and deterministic fixture. Faults requiring private injection
run the same terminal-host implementation in a child test binary rather than
adding a production fault flag. The Expect version and terminal environment
are pinned by the controlled job and printed in evidence; Expect and Tcl never
enter Cargo metadata or the shipped bundle.

### Web handler and real-browser regression

`PAR-WEB` keeps every merge-base Rust web test and adds handler-level exact
shape tests for Volume, Sector and Page resources before/after enrichment,
including duplicate top-level Slots/distribution where currently observable,
allocation/occupancy, file/class/table attribution, diagnostics/coverage,
record interpretation added after `cba72cd`, and path-free disclosure. Preserve
exact revision envelopes, bounded pagination/cursors, `202` receipt,
`Location`/result revision and URL, old-revision access, structured stale,
invalidation, admission/resource and unsupported conflicts, and successful
diagnostic-bearing revisions.

Source-string assertions in embedded assets are not browser evidence. A
separately locked Playwright package and its pinned Chromium revision launch
the exact candidate static-musl server against the canonical corpus. The
semantic browser suite covers:

- complete Volume mosaic, bounded loading and first/middle/last Sector access;
- replacement Volume -> Sector -> Page screens, direct deep URLs,
  breadcrumbs, Back/Forward and reload at exact revisions;
- all 64 Sector Pages, known/unknown occupancy, Page/Sector attribution and
  findings;
- exhaustive fragmented distribution, every Slot state and current record
  interpretation states without payload-byte display;
- enrichment loading, exact returned revision/URL, old history, conflicts and
  diagnostic revision; and
- terminal invalidation overlay on retained old routes.

Assertions use stable roles, labels, typed data attributes, canonical URLs and
semantic counts. CSS class names, pixel positions, screenshots and browser
paint timing are not compatibility assertions. Playwright, browser revision
and package integrity live in an isolated test lockfile, are reviewed as test
supply chain, and do not enter the production Cargo graph or release bundle.

### Findings, coverage, and non-disclosure

Findings tests cover every severity, Page/Sector/Volume/Slot/OOS affected
reference, global and unresolved references, deterministic occurrence order
and wrap, filter independence, ancestor landing, Finding detail, and misleading
human strings. Coverage tests cover trusted complete totals, partial prefixes,
unknown totals/remainders, resource/cancel boundaries, diagnostic containment
and terminal invalidation. TUI assertions prove these remain reachable in
Findings/Coverage at all tiers without moving or mutating allocation facts.

`PAR-DISCLOSURE` uses different unique sentinels for forbidden raw application
payload, ciphertext, TDE key bytes, key/input paths, control/ANSI injection and
an explicitly allowed typed value. It scans:

- Projection-safe facts and diagnostics;
- every Atlas scene, notice, status and error;
- normalized cells and encoded terminal output under all profiles;
- web JSON, semantic DOM, browser error display and URLs;
- CLI JSON/JSONL and deterministic HTML exports; and
- controlled PTY/browser logs and generated golden/benchmark evidence.

Forbidden sentinels must be absent, while expected structural identity, byte
range, type, allocation, coverage and `[ESC]`/`[U+XXXX]` sanitization must be
present so a blank output cannot pass. The allowed typed value appears only in
the existing explicitly targeted record-interpretation surfaces and never
authorizes raw or undecodable bytes. This ticket does not broaden explicit-
target disclosure or add record-value parity to the three Atlas screens.

### Large-volume, memory, redraw, and latency proof

`PAR-RESOURCE` separates terminal rendering budgets from the existing
Inspection benchmark. Ordinary deterministic tests assert:

- no more than 32,768 active cells, 64 resident complete Sector cards, 48
  visible-plus-overscan cards, 384 prepared Page rows, two cell frames, one
  dirty frame and one progress item;
- every Projection query is contiguous and bounded to 64 Sectors, traversal of
  257 Sectors is exhaustive, and work/residency is identical for small and
  33,554,432-Sector topologies at the same window;
- one-row overscan, incremental prefix/suffix refill, atomic distant refill,
  exact focus/anchor restoration and the full invalidation matrix;
- at most 60 interactive and ten progress frames per second under scripted
  monotonic time, with zero idle frames and immediate terminal completion; and
- cache pressure never changes Inspection coverage, diagnostics, outcome,
  revision or reachability.

Atlas owns an explicit memory ledger. Before retaining a value, it accounts for
vector/string capacities, reservoir summaries, cell storage, semantic styles,
prepared rows, text/raster caches and geometry using checked arithmetic. Every
construction, refill, scroll, resize, profile/filter change, overlay, Page
maximum and revision adoption asserts the ledger is at most 16 MiB and returns
to the expected weight after invalidation. Process RSS is diagnostic only: it
includes Projection storage, allocator/runtime state and OS buffers excluded by
ticket 07 and cannot prove or override the ledger.

The controlled release benchmark emits machine-readable evidence containing
candidate commit, toolchain, target, host/CPU identity, workload/fixture,
profile/extent, warm-up and sample counts, every measured sample, percentiles,
work cardinalities and memory-ledger peaks. It uses at least
10,000 warm and 500 cold samples and directly gates ticket 07's thresholds:

| Path | Required latency |
| --- | ---: |
| Cold maximum-canvas composition | p99 <= 25 ms |
| Warm focus or one-row scroll composition plus diff | p95 <= 8 ms; p99 <= 16 ms |
| Input receipt through successful `LayoutCommit`, excluding terminal blocking | p95 <= 33 ms; p99 <= 50 ms |

The controlled PTY run reports key-read through successful flush separately.
There is no checked-in timing golden. A failed valid threshold run fails that
candidate; rerunning may diagnose infrastructure but cannot turn the failed
artifact into a pass. Acceptance requires a fresh valid run. Ordinary CI uses
deterministic operation/card/row/cell/byte counts and contains no timing sleeps.

### Blocking jobs, release proof, and failure policy

Gate classification is explicit:

| Classification | Required evidence |
| --- | --- |
| Ordinary locked CI | Projection contracts, Atlas traces/properties, 36 core and focused goldens, renderer geometry/text/faults, scripted host, memory ledger, disclosure, Rust web handlers, existing CLI/JSON/export tests |
| Controlled blocking jobs | Static-musl Expect PTY suite, locked Playwright/Chromium suite, reference-host latency report, full dependency/reproducibility/static/cross-distribution release audit |
| Review aids only | Browser/terminal screenshots, videos, raw ANSI transcripts, RSS, flamegraphs, prototype captures and uncontrolled timing |

Every required job names the exact same candidate commit. Missing or invalid
infrastructure means acceptance is not proven; it is distinguishable from a
product assertion failure but does not become a pass. Automatic retries cannot
mask a failure. Test quarantine, weakened assertions, ignored cases, increased
budgets, removed profiles/tiers or golden rebaselining require an explicit
product decision rather than a test-only workaround.

Retain `cargo fmt --check`, all locked debug and release tests, Clippy with zero
warnings, exact dependency pins, `cargo deny`, deterministic notice and
CycloneDX SBOM regeneration, two-checkout byte reproducibility, static
x86_64-musl ELF checks and Debian/Rocky/Alpine execution. The distribution
audit adds the same non-TTY `tui` rejection in all three images; full
interactive behavior runs once on the controlled Linux PTY because the exact
static binary and terminal-host semantics are otherwise identical. The
existing Inspection resource matrix remains a separate gate.

Do not add a Rust snapshot, property, mock, async-test or PTY framework by
default. Repository-owned serializers, seeded generators/reducer, scripted
host and real in-process modules are sufficient for the closed Atlas
vocabulary. A later test dependency is permitted only when its exact graph
passes license, duplicate-version, musl, notice, SBOM and reproducibility
review and it replaces meaningful private complexity rather than wrapping a
single call.

### Cutover acceptance and legacy deletion

Development may keep Atlas beside the legacy TUI behind non-production or
test-only construction. Terminal interaction parity is achieved only when one
exact candidate commit:

1. passes every gate family and blocking job above;
2. routes production `tui::run` exclusively through the accepted
   Projection workspace, `AtlasMachine`, `AtlasRenderer` and terminal host;
3. contains no production translation from legacy state into Atlas state and
   no second renderer;
4. deletes the legacy `State`, direct clear/draw path, fixed-coordinate mouse
   reconstruction, eager `detail_lines`, scalar-count truncation,
   terminal-too-small exit and obsolete four helper tests; and
5. preserves every merge-base web, CLI, JSON/JSONL, deterministic HTML,
   disclosure and release assertion without silently rebaselining it.

The parity matrix and golden artifacts are versioned review evidence, not a
license to freeze implementation details. A deliberate accepted behavior
change updates its owning Wayfinder decision or later product specification
first, then changes the named assertions and reviewed goldens. Tests never
define new storage facts by accident.

No new domain term is added: `Terminal interaction parity`, `Projection
workspace`, `Atlas trail`, `Terminal presentation profile`, and `Terminal
rendering budget` already name every durable concept. Gate ids, fixtures,
goldens, test hosts and evidence artifacts are verification implementation
vocabulary rather than inspection-domain language.

No new ticket is created. Ticket 09 can now assemble the accepted hierarchy,
shared seam, state model, renderer, enrichment lifecycle, semantic vocabulary,
resource policy and this verification contract into the implementation-ready
specification.
