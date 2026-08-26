# Volmap interactive live web implementation specification

Status: implementation-ready

Source baseline: `893bfd9` (`docs: add TUI parity implementation tickets`)

Prototype evidence: `f42c790` on `prototype/runtime-observation-ui` (design evidence only; do not merge)

CUBRID producer dependency: work-tracker item `27` and its reviewed cross-repository handoff. That effort owns the CUBRID branch, gate, wire, socket, and producer-test contract. This specification owns Volmap's consumer boundary and browser behavior.

## How to use this specification

This file is the implementation index and delivery plan for replacing only the live browser viewer with a pinned React/TypeScript application, adding evidence-backed attribute byte selection, and adding optional runtime observations from the Linux page cache and a cooperating `cub_server`.

The linked ADRs and delivery tickets are normative. Private file layout and helper names may change, but the source boundaries, state invariants, disclosure rules, resource limits, and verification gates below may not be weakened in implementation. A conflict with the finalized item-27 producer handoff must be resolved at the Volmap consumer adapter; it must not be hidden by teaching the browser the producer protocol.

The deterministic HTML export remains a separate frozen renderer. The TUI, CLI, JSON/JSONL, and inspection graph keep their existing contracts unless a ticket below explicitly expands a presentation-neutral projection.

## Decision index

| Concern | Normative decision |
| --- | --- |
| Live frontend and release graph | [ADR-0003: Build the live viewer from pinned React and TypeScript sources](../../docs/adr/0003-react-typescript-live-viewer.md) |
| Runtime observation versus resident inspection | [ADR-0004: Separate lightweight runtime observation from resident-page inspection](../../docs/adr/0004-separate-runtime-observation-from-resident-inspection.md) |
| Byte-coordinate ownership | [ADR-0005: Project byte coordinates before rendering](../../docs/adr/0005-project-byte-coordinates-before-rendering.md) |
| Runtime capability and exposure boundary | [ADR-0006: Treat runtime observations as loopback web capabilities](../../docs/adr/0006-runtime-observations-are-loopback-web-capabilities.md) |
| Existing live-follow generations | [Live volume follow](../live-follow/SPEC.md) |
| Existing producer design direction | [Live page-buffer inspection](../../docs/live-page-buffer-inspection.md) and tracker item `27` |
| Record interpretation and disclosure | [Record interpretation specification](../record-interpretation/SPEC.md) and [ADR-0001](../../docs/adr/0001-explicit-target-disclosure.md) |
| Shared Volmap domain vocabulary | [`CONTEXT.md`](../../CONTEXT.md) |

## Scope

The production live viewer becomes a React 19 and TypeScript application built by an exact, locked Node/pnpm/Vite toolchain. Its generated production assets remain committed and embedded in the single Volmap executable, so an ordinary Rust checkout and build require no JavaScript toolchain.

The viewer adds:

- keyboard- and pointer-equivalent attribute selection;
- synchronized record-relative, page-content, physical-page, and volume-file byte extents projected by Rust;
- exact NULL, metadata-anchor, OOS-inline, withheld-value, and relocation behavior;
- one optional runtime overlay at a time over the existing allocation, occupancy, and finding display;
- bounded state-only page-buffer observations through a protected local `cub_server` attachment;
- explicit selected-page resident structure inspection, separately requested from state polling;
- bounded Linux page-cache residency observations using `cachestat(2)` where supported and `mincore(2)` as the fallback;
- truthful capability, freshness, scope, partial-coverage, pause, restart, and image-correspondence states.

## Non-goals

- React does not replace `export html`, the TUI, CLI, or JSON/JSONL renderers.
- Three.js, WebGL, canvas-first rendering, a generic visualization framework, Redux, Zustand, and TanStack Query are not introduced in version one. The current rectangular maps are DOM/SVG-scale interactions, not a 3D scene.
- Runtime readings do not enter the inspection graph, immutable revisions, snapshot generations, exports, diagnostics, outcomes, or terminal-parity contract.
- Volmap does not expose the CUBRID inspector socket to the browser, proxy arbitrary producer messages, or accept a remote runtime attachment.
- Page-buffer state does not claim transaction visibility, committed state, filesystem synchronization, or physical durability.
- Kernel-cache version one reports residency only. It does not infer OS dirty, writeback, eviction cause, access frequency, or a history from residency samples.
- No runtime surface returns raw page bytes, application values, TDE material, memory addresses, private C structures, holder/thread identities, or lock-owner data.
- The browser never reconstructs CUBRID storage-format arithmetic or producer wire semantics.

## Accepted architecture

```text
validated volume facts                    optional volatile sources
Inspection / GraphView                    +---------------------------+
          |                               | cub_server UDS (item 27)  |
          v                               | Linux kernel cache probe  |
byte-coordinate projection               +-------------+-------------+
          |                                             |
generation-pinned JSON resources                         v
          |                                  Volmap runtime broker
          +----------------------+----------------------+
                                 |
                     same-origin Axum resources
                                 |
                     typed browser effect adapters
                      HTTP / timer / visibility /
                      history / abort-controller
                                 |
                                 v
                      pure RuntimeUi reducer
                     + deterministic selectors
                                 |
                                 v
                     React semantic components
             mosaic / page map / record map / tables
```

The architecture has five deep seams:

1. **Byte-coordinate projection** owns every format-dependent transform and returns closed typed extent and anchor facts. It accepts validated inspection facts and never accepts browser coordinates.
2. **Runtime broker** owns attachment policy, identity binding, producer translation, kernel probing, limits, and source timestamps. It does not mutate or publish inspection revisions.
3. **Browser model** is one pure reducer plus deterministic selectors. It owns view state and response-adoption authority, but performs no I/O and no disk-format arithmetic.
4. **Effect adapters** translate reducer effects into same-origin requests, timers, document visibility, aborts, and existing browser history. They return typed actions stamped with the scope that authorized them.
5. **React views** render semantic state and dispatch semantic actions. DOM geometry and styling never become state identity.

The item-27 producer seam ends at the protected Unix socket. Volmap alone maps the reviewed producer protocol into its private `PageBufferObservationAdapter`; neither producer field names nor transport frames cross the HTTP boundary unless they are already accepted Volmap domain terms.

## Cross-source invariants

- Disk generations, page-buffer captures, resident inspections, and kernel-cache captures are independent observations with independent timestamps.
- No multi-page runtime response is presented as an atomic database snapshot.
- A VPID match is identity, not page-image correspondence.
- Resident and disk geometry may be combined only when correspondence is proven for the displayed disk generation and the exact resident capture token. Divergent or unknown images remain side-by-side.
- Advancing a disk generation clears resident correspondence and structure derived from an older disk comparison. It does not clear a state-only page-buffer observation solely because the disk generation changed.
- Changing route, visible scope, overlay, pause epoch, database identity, producer incarnation, or request epoch revokes adoption authority for incompatible in-flight responses.
- A producer restart clears only incarnation-bound runtime data. It does not navigate away from or rewrite the current disk inspection.
- Pausing freezes adoption of both newer disk generations and runtime captures. Offers may accumulate, and resume adopts coherent latest offers before repainting.
- A hidden document starts no new runtime work. Returning visible schedules fresh work rather than replaying every missed interval.
- Runtime failure never degrades ordinary inspection. It changes a capability state and retains only still-valid, explicitly aged evidence.

## Byte-coordinate projection contract

### Types

Every new coordinate is numeric JSON, not a decimal string and not a CSS percentage:

```text
ByteExtent {
  origin: record | page-content | physical-page | volume-file,
  start: u64,
  length: u64,
  end_exclusive: u64
}

BytePoint {
  origin,
  offset: u64
}

MetadataAnchor {
  kind: bound-bit | variable-offset-start | variable-offset-end | oos-inline-prefix,
  extent: ByteExtent,
  bit_index?: u8
}

StoredAttributeExtent {
  identity: { record_oid, representation_id, attribute_position },
  storage: fixed | variable,
  value_state: decoded | null | out-of-row | withheld,
  record: ByteExtent | BytePoint,
  page_content?: ByteExtent | BytePoint,
  physical_page?: ByteExtent | BytePoint,
  volume_file?: ByteExtent | BytePoint,
  anchors: MetadataAnchor[],
  page_geometry: direct | relocation-target | target-not-loaded
}
```

`end_exclusive` is redundant by design and is checked against `start + length` at construction and deserialization. It makes the interval convention reviewable at the API boundary. A zero-length variable NULL uses `BytePoint`; renderers must not inflate it into an invented byte extent.

### Rules

- The enclosing interpreted attribute extent is authoritative. Normalize or remove the existing body-relative duplicate `AttributeValueProjection::Withheld.offset/length` before a browser can consume it.
- Fixed NULL keeps its nonzero fixed-region extent and gets its exact bound-bit byte/bit anchor.
- Variable values get both offset-table-entry anchors. Variable NULL has an exact insertion point plus those anchors.
- OOS selection highlights the complete proven inline stored attribute extent and the semantic relationship to the OOS head. It does not label all inline bytes as the fixed-size OOS prefix and does not project the logical value across OOS pages.
- A relocation response may render its target-record-relative extent immediately. It must not project that extent onto the source page. Page and file coordinates appear only after the target slot geometry is loaded and identified.
- Page-content coordinates use the slotted page's validated coordinate origin. Physical-page and volume-file coordinates are projected in Rust from pinned constants and the page identity.
- The browser resolves a committed selection after refresh only by record OID, representation id, and attribute position. A mismatch clears it with an explicit reason; name or array index alone is insufficient.

Projection tests pin ordinary fixed, fixed NULL, variable, variable NULL, OOS, withheld, malformed, and relocation source/target examples in all available coordinate systems.

## Browser state and effects contract

### State partitions

The reducer state is closed and serializable:

```text
route             current semantic entity and existing browser-history identity
disk              displayed generation, offered generation, validity, age
selection         committed identity, preview identity, resolved projected facts
display           paused, hidden, overlay, visual encoding, reduced-motion mode
capabilities      page-buffer, resident-inspection, kernel-cache source states
observations      latest accepted state-only batches by source
resident          selected-page capture, structure, correspondence, limitations
coverage          requested/evaluated/budget/rotation for each batch source
requests          monotonically increasing epoch and in-flight scope keys
announcement      sparse accessibility announcement, separate from status text
```

Capability is one of `disabled`, `connecting`, `active`, `stale`, `unavailable`, `refused`, or `incompatible`. Freshness is a derived selector: fresh through two expected intervals, stale after that, always accompanied by age and capture time. `unavailable`, `refused`, and `incompatible` are not rendered as `not resident`.

### Actions and effects

Semantic actions include route change, preview/commit/clear attribute, disk offer/adopt, pause/resume, visibility change, overlay/encoding change, capability transition, request start/success/failure, producer restart, resident inspection request/result, correspondence result, and bounded-batch rotation.

Every runtime request uses an immutable `ObservationScopeKey` containing at least source kind, request epoch, database identity, producer incarnation when applicable, selected VPID, ordered requested VPIDs or their stable scope digest, overlay, and pause epoch. A success is adoptable only if its returned key equals the current authorized key and the corresponding request remains in flight. Aborting a fetch is an optimization; the reducer guard is the authority.

The reducer returns state and ordered effect intents. Effect adapters own `fetch`, `AbortController`, clocks, timers, `document.visibilityState`, and History API calls. Reducer and selectors are importable without React or DOM and run under deterministic tests.

Preview is transient and never enters URL/history. Click, Enter, or Space commits. Escape clears. Poll ticks are not announced. The live region announces committed selection changes, pause/resume, capability transitions, producer restart/handshake, and correspondence transitions.

## Runtime HTTP resources

Runtime resources are distinct from generation-pinned graph resources and carry `Cache-Control: no-store`. They use a private Volmap version-one schema; the browser never calls the producer socket.

```text
GET  /api/v1/runtime/capabilities
POST /api/v1/runtime/page-buffer/observe
POST /api/v1/runtime/page-buffer/inspect
POST /api/v1/runtime/kernel-cache/observe
```

All requests are bounded and identity-bound. Observation requests name an ordered VPID set, selected VPID, request epoch, and scope digest. The server enforces its own smaller hard cap even if the browser asks for more. Responses echo the accepted scope, report requested/evaluated counts and a rotation continuation, and carry source capture time, method, limitations, and per-page semantic states.

`page-buffer/observe` is state-only: it must not load a missing page, copy page content, hash images, perform disk I/O, or wait unboundedly for page protection. Its normalized per-page result can express residency, fixed/unfixed state, semantic latch state, dirty, flushing, page LSA, capture token, and individual limitations, subject to the finalized item-27 wire contract.

`page-buffer/inspect` is an explicit selected-page operation. It may return sanitized resident slotted-page structure and correspondence evidence, but never raw bytes or values. It is not triggered by the volume poll loop. Its response identifies the resident capture, disk generation compared, relation (`matching`, `divergent`, or `unknown`), and why the relation is limited.

`kernel-cache/observe` reports only `fully-resident`, `partially-resident`, `not-resident`, or `unknown` for each requested physical volume-page range. It also reports the probe (`cachestat`, `mincore`, or unsupported), kernel capability, capture time, and limitations.

HTTP error mapping preserves semantic distinctions: disabled, unavailable socket, peer refused, identity mismatch, protocol incompatibility, deadline, and resource limit do not collapse into one generic fetch failure.

## Runtime attachment and security

- `serve` takes an explicit producer socket path and an explicit runtime-observation enable option. There is no socket discovery, environment-variable fallback, or implicit attachment merely because a socket exists.
- Runtime attachment is accepted only when the HTTP listener is loopback. A wildcard or non-loopback bind and runtime attachment is a startup error, not a warning or silent downgrade.
- Volmap opens the configured database normally, then requires the producer handshake to prove the same database and volume identities and a server incarnation before any observations are accepted.
- The producer socket and peer policy follow item 27. Volmap treats a failed peer/identity check as `refused`, clears producer-derived state, and keeps disk inspection available.
- Remote users forward the loopback HTTP listener through SSH. Version one does not add HTTP authentication or TLS.
- CSP stays same-origin with no inline script, remote code, `eval`, WebSocket, or producer connection. Runtime calls use ordinary same-origin HTTP.
- Every runtime response and log line passes the existing disclosure/path-redaction policy. Structural-only negative sentinels cover producer payloads and in-memory plaintext.

## Polling, batching, and resource limits

- Selected-page state cadence defaults to 500 ms.
- Visible-sector/page state cadence defaults to 2 s.
- Kernel-cache observations default to the visible cadence; the selected page remains first in every request.
- Hidden documents stop scheduling. Paused displays stop adoption and do not build an unbounded queue; at most the latest offer per source is retained.
- Failures use capped exponential backoff with jitter and reset after a successful handshake/sample. Capability age still advances while backing off.
- The illustrative and production version-one browser request cap is 512 pages. The server may enforce a lower cap based on byte/time budgets.
- Ordering is selected page first, then pages in visible sectors nearest the selected page, then remaining visible pages in physical order.
- When requested pages exceed admission, UI and response show exact requested/evaluated counts. Subsequent polls rotate the non-selected portion; sampling is never silent.
- The browser retains only the latest accepted observation per page/source/incarnation and no event history. Route and viewport caches remain bounded independently of volume size.

## Linux kernel-cache adapter

Kernel probing is a private adapter outside the inspection graph. It uses non-loading range queries against the already identified volume files:

1. On Linux kernels providing `cachestat(2)`, query each coalesced requested range and classify full/partial/none from cache counts.
2. Otherwise use `mincore(2)` over page-aligned, read-only mappings without touching mapped bytes.
3. On unsupported platforms, permission errors, unrepresentable mappings, truncated files, or unstable file identity, report `unknown` or a source capability state; never guess `not-resident`.

The main `volmap` crate keeps `unsafe_code = "forbid"`. If no reviewed safe Rust API covers both probes, isolate the minimal Linux FFI in one private workspace adapter crate with a documented safety proof and focused tests. That crate exposes only safe range-classification operations and contains no CUBRID format logic. The release audit treats it as product code, and non-Linux builds compile an unsupported adapter.

The adapter does not open a second path based on browser input. It receives validated volume identities/handles and checked `u64` ranges from Volmap. It coalesces adjacent requested physical pages, bounds mapped/query bytes, unmaps deterministically, and detects file replacement before adopting results.

## Visual and accessibility contract

Allocation/occupancy/finding semantics remain visible regardless of runtime overlay. Exactly one of `none/allocation`, `page-buffer`, or `OS cache` controls the additional runtime encoding.

Three implementations are evaluated against the same state during migration: border plus badge, tint plus pattern, and split cell. The final default is border plus badge because it preserves the allocation fill; the other encodings may remain as a diagnostic comparison control only if they meet contrast and density gates. No state is color-only: every runtime condition has a glyph or text label, accessible page name, contextual legend, and selected-page detail.

Hover and focus preview the same stored extent. Commit styling is visually distinct and persists until clear or invalidation. Reduced-motion mode removes animated freshness/transition effects. Focus order follows the semantic hierarchy. Virtualization must not make a committed selection unreachable or erase the selected-page textual state.

Screen-reader announcements are sparse: committed selection, capability, pause, and correspondence changes are announced; periodic sample arrival and age ticks are not. All controls remain operable with keyboard only, including selecting a page, selecting/clearing an attribute, changing an overlay, requesting resident inspection, and reading coverage/limitations.

## React and asset build contract

- Production runtime dependencies start with only `react` and `react-dom`. Add a dependency only when a native browser API or small local module cannot satisfy a measured need.
- Node, pnpm, TypeScript, Vite, React, React DOM, test runner, and Playwright are exactly pinned. The lockfile is immutable in CI; lifecycle scripts are disabled unless individually reviewed.
- TypeScript uses strict mode, no implicit `any`, checked indexed access, and an explicit browser target. Generated API types come from repository-owned schemas or are checked manually; runtime validation is required at the HTTP boundary.
- Vite emits deterministic same-origin JS/CSS with no remote chunks, source-map path leakage, runtime CDN, service worker, or inline execution. Stable embedded asset routes are preferred because every response is `no-store`; a generated manifest is the Rust embedding source of truth.
- Generated production assets are committed. Ordinary `cargo build` embeds them and does not invoke Node. A dedicated regeneration command writes only the generated asset directory and manifest.
- Release verification starts from two clean archives, installs the exact JavaScript graph in a controlled environment, regenerates assets, and byte-compares both the committed bundle and final static executable.
- Bundled runtime packages enter product notices and the CycloneDX artifact SBOM. Build-only JavaScript packages enter a separate provenance/advisory/license report so the shipped artifact is not falsely described as containing the complete build graph.
- The existing deterministic HTML export remains byte-stable and does not import the React bundle.

## Verification contract

The implementation adds stable gate families:

| Gate | Authority |
| --- | --- |
| `WEB-PROJECTION` | Rust projection tests for all byte coordinate/anchor/relocation cases and disclosure |
| `WEB-MODEL` | deterministic reducer, selector, scope-key, pause, restart, stale, and late-response traces |
| `WEB-HTTP` | Axum tests for routes, identity policy, caps, semantic error mapping, CSP, and no-store |
| `WEB-BROWSER` | Playwright Chromium against the actual Rust server for hierarchy, history, byte selection, overlays, polling, pause, visibility, direct routes, and accessibility |
| `WEB-FIREFOX` | a smaller Firefox smoke covering bootstrap, navigation, byte selection, and one runtime overlay |
| `WEB-KERNEL` | safe-adapter unit tests plus supported/unsupported Linux integration evidence without asserting host cache luck |
| `WEB-PRODUCER` | simulated adapter contract tests and, after item 27, exact cross-repository wire/identity integration |
| `WEB-DISCLOSURE` | positive structural sentinels and negative raw/value/path/private-state sentinels across HTTP, DOM, logs, bundles, and snapshots |
| `WEB-RELEASE` | immutable npm graph, notices/SBOM/provenance, regenerated-bundle equality, CSP, static binary, export determinism, and two-archive reproducibility |

Reducer and selector tests are mandatory but do not replace production-browser tests. Chromium is blocking; Firefox is a blocking smoke. Browser assertions target semantic roles, accessible names, URLs, typed state, and textual facts rather than incidental CSS class names or screenshots. Screenshots remain review evidence only.

The runtime browser suite uses an in-memory deterministic observation adapter capable of active, stale, refused, incompatible, restart, divergent, partial-coverage, delayed, and out-of-order scenarios. Tests control time and visibility. They prove the selected 500 ms and visible 2 s schedules without sleeping on wall time.

## Dependency-ordered delivery plan

```text
W0 -> {W1, W2}
{W1, W2} -> W3
W1 -> W4
W4 -> W5
W5 -> W6
{W5, item-27 handoff} -> W7
{W3, W7} -> W8
{W3, W4, W6, W7, W8} -> W9
{W0 ... W9} -> W10
```

### W0 — Freeze frontend and release evidence

Add the exact JS toolchain, deterministic asset generation, immutable install, generated-asset diff, supply-chain reports, and real browser harness before changing production rendering.

Completion: a minimal committed bundle is byte reproducible from two clean archives; Chromium and Firefox launch through repository-owned commands; Cargo-only ordinary builds still work; export bytes are unchanged.

### W1 — Establish the React compatibility viewer

Recreate the current live viewer's routes, progressive loading, navigation/history, live-follow, pause, enrichment, direct loads, licenses, and disclosure behavior in React/TypeScript. Keep a pure reducer/selector seam from the beginning.

Completion: current web HTTP and behavior corpus plus browser parity passes against React; no storage arithmetic or runtime observation is added yet.

### W2 — Project typed byte coordinates

Normalize withheld coordinates and add the Rust-owned stored-extent, point, anchor, physical-page, file, and relocation-target projection with exhaustive fixtures.

Completion: `WEB-PROJECTION` covers every accepted attribute/storage case and no browser calculation is necessary.

### W3 — Add attribute selection and cross-highlighting

Consume W2 through semantic React components, stable selection identity, hover/focus preview, keyboard commit/clear, record/page maps, all four coordinate readouts, NULL/OOS/relocation rules, and sparse announcements.

Completion: Chromium and Firefox exercise the exact interactions and refresh preservation/clearing rules with disclosure sentinels.

### W4 — Add the runtime state machine and simulated source

Implement capability/freshness/coverage/request state, scope keys, pause/visibility/restart/late-response rules, effect interfaces, and a deterministic in-memory adapter. Add the one-overlay semantic UI with all three candidate encodings.

Completion: `WEB-MODEL` proves every transition and illegal join; browser scenarios match the accepted prototype without production attachment.

### W5 — Add the loopback runtime broker and HTTP boundary

Add explicit serve configuration, loopback enforcement, database/volume identity binding, private adapters, bounded runtime routes, error mapping, and cancellation/backpressure. Initially serve only simulated/disabled adapters.

Completion: `WEB-HTTP`, security, CSP, direct-route, and disclosure gates pass; runtime failure cannot affect graph resources.

### W6 — Add Linux page-cache residency

Implement the checked range projection, `cachestat`/`mincore`/unsupported adapters, page batching/coalescing, capability mapping, and OS-cache overlay.

Completion: supported and unsupported paths pass without loading data or claiming dirty/writeback state; the main crate remains unsafe-free.

### W7 — Consume the finalized CUBRID state contract

After tracker item 27 publishes its reviewed handoff, implement the UDS consumer and normalized state-only batch adapter. Do not copy producer framing or engine vocabulary into browser components.

Completion: simulated and real-producer contract tests prove handshake, identity mismatch, incarnation restart, caps, timeouts, semantic states, and structural-only disclosure.

### W8 — Add selected resident-page inspection

Add the explicit request, sanitized resident structure, capture identity, disk/resident correspondence, and side-by-side divergent/unknown presentation. It remains separate from state polling.

Completion: matching geometry combines only with proof; divergent/unknown/restarted/advanced cases cannot cross-highlight resident structure.

### W9 — Close scheduling, density, and accessibility

Implement selected/visible cadence, hidden pause, backoff, explicit overflow, rotating priority batches, bounded retention, final overlay encoding, reduced motion, keyboard reachability, and sparse announcements under large-volume fixtures.

Completion: `WEB-MODEL`, `WEB-BROWSER`, disclosure, accessibility, and resource checks pass at cap boundaries and with out-of-order work.

### W10 — Prove release and remove the legacy viewer

Run all gates on one exact candidate, remove the unserved hand-written live JS/CSS and obsolete source-text tests, regenerate notices/SBOM/provenance, and prove the static artifact and HTML export.

Completion: all `WEB-*` gates and existing Rust/release/distribution gates pass on the same post-deletion commit; production serves only the generated React viewer and contains no second browser state model.

## Handoff audit

- All product, source-ownership, interaction, security, freshness, pause, resource, disclosure, accessibility, migration, and verification decisions required for Volmap implementation are resolved.
- The exact CUBRID wire and producer implementation remain intentionally owned by tracker item 27. W7 begins only from its reviewed handoff; that is a dependency, not an unresolved Volmap decision.
- The prototype is disposable interaction evidence and must not be merged into production.
- HTML export and TUI runtime-overlay parity are explicitly out of scope for version one.
- The tickets in `issues/` are the implementation handoff. Production code should be changed ticket-by-ticket, with tests written before or with each slice.
