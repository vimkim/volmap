# Volmap seams for a live page-buffer overlay

Charting survey, 2026-08-21. Surveyed tree: this repository at commit `1e90ae8`.
Question: where would a volatile, VPID-keyed buffer-state overlay attach, and
which documented contracts does it touch?

## Q1 — How the web adapter renders volume / sector / page views

Transport shape: JSON API + compile-time-embedded static assets; no server-side
HTML for any view.

- Router: `src/web.rs:211-247`. Every browser drill URL returns the same
  `index.html` (`src/web.rs:222-227`); all facts arrive over `/api/v1/*`.
- Assets are `include_str!`-embedded (`src/web/assets.rs:5-10`), served with
  fixed media types (`assets.rs:31-50`); `index.html` loads `routes.js`,
  `distribution.js`, `app.js` (`src/web/assets/index.html:34-36`).
- Data endpoints: paginated mosaic `GET /api/v1/sectors/{vol}`
  (`src/web.rs:622-684`, page size default 24, max 64 — `src/web.rs:54-55`),
  single sector `src/web.rs:1024-1039`, page workspace `src/web.rs:1041-1086`
  returning `PageResourceProjection { page, deep, slots, distribution }`
  (`src/web.rs:1088-1094`).

Per-page semantics are computed in Rust as typed enums, then mapped to CSS
classes in the browser:

- `PageProjection` (`src/projection.rs:200-214`) carries independent
  dimensions: `allocation`, `page_type`, `availability`, `tde_state`,
  `detail_support`, `occupancy`, `lsa_word`, `diagnostic`, `file_association`.
  Built by `page_projection` (`src/projection.rs:850-889`) from `PageView`
  (`src/inspection.rs:226-239`).
- Occupancy is already a continuous per-page scalar:
  `PageOccupancyProjection::{Unknown, Known{occupied_percent, free_percent}}`
  (`src/projection.rs:244-252`).
- Browser: mosaic cells `page preview-page ${page.allocation}` + `finding`
  (`src/web/assets/app.js:817-823`), 64-page sector grid
  (`app.js:883-908`), and `applyPageFill` adds `occupancy-known` + an
  `--occupied` CSS custom property or `occupancy-unknown` (`app.js:497-506`).
- CSS: cell base/classes `src/web/assets/app.css:247-284` (occupancy gradient
  `:265-278`), legend swatches `:169-190`; legend markup inline in JS
  (`app.js:420`).

Heatmap precedents: (1) the volume mosaic itself — a page grid colored by
allocation class with per-cell occupancy gradient (`app.js:783-826`,
`app.css:265-278`, `README.md:143-153`); (2) the slotted-page byte
distribution strip (`src/web/assets/distribution.js:4-55`); (3) record
byte-layout bands (`app.js:532-589`).

Other adapters rendering the same facts: TUI grid markers `S`/`r`/`.`/`!`
with no occupancy dimension (`src/tui.rs:622-649`, legend `:615-620`); CLI map
markers (`src/cli.rs:1160-1175`); HTML export has its own inlined mosaic
CSS/JS pinned by sha256 in a CSP meta tag (`src/export.rs:243`, `:246`).

## Q2 — Live follow end-to-end, and where a second source attaches

Change detection is file-stamp polling only: `source::fingerprint` reads the
`_vinf` manifest and per-volume stamps without opening volumes
(`src/source.rs:62-111`).

Follower + generation store (`src/follow.rs`):

- Tuning (500 ms poll, 300 ms quiet, 3 s max defer, 250 ms idle floor,
  retain 4): `follow.rs:26-50`; pure decision `should_rescan`: `:71-79`.
- `LiveSource` owns `generations: BTreeMap<u64, Generation>`, `current`,
  `change_pending`, and a `tokio::sync::watch::Sender<u64>`
  (`follow.rs:109-161`); `subscribe` `:175`, `publish` (new generation +
  eviction + notify) `:233-271`, `publish_revision` (enrichment inside a
  generation) `:278-294`.
- `Reading` = generation, view, validity, `observed_at_unix_seconds`,
  `input_modified_unix_seconds`, scan duration (`follow.rs:95-107`).
- The loop: `follow()` `follow.rs:319-415` — `spawn_blocking` fingerprint,
  debounce, then a full `Inspection::open_live` rescan (`:386-395`).
  Incremental rescan was explicitly rejected
  (`.scratch/live-follow/SPEC.md:163-166`).

Publication to the browser is long-poll, not SSE, not reload:

- `WebState { source: Arc<LiveSource>, … watchers: Semaphore }`
  (`src/web.rs:67-76`); follower spawned at `src/web.rs:130-136`.
- `GET /api/v1/live/watch?generation=N` holds up to `WATCH_TIMEOUT` = 25 s
  (`follow.rs:23`) returning `WatchProjection { advanced, follow }`
  (`src/web.rs:539-584`). SSE was deliberately avoided because the release
  dependency graph is pinned (`src/web.rs:550-556`,
  `.scratch/live-follow/SPEC.md:126-131`).
- Watchers have their own admission pool (`MAX_CONCURRENT_WATCHERS = 64`,
  `src/web.rs:50`, routed `:266-270`) separate from inspection requests
  (`MAX_CONCURRENT_REQUESTS = 32`, `:47`).
- Every envelope carries generation/observation/disk time: `api_envelope`
  `src/web.rs:1608-1641`, `SnapshotProjection` `src/projection.rs:34-53`;
  superseded standing is annotated, never failed (`src/web.rs:464-493`).
- Browser: `followLoop` long-poll (`app.js:248-268`),
  `refreshCurrentDrillLevel` (`:176-187`), Pause (`:277-282`), header chip
  (`:155-169`).
- Collection cursors are MAC'd over `generation || offset`
  (`src/web.rs:916-974`); older generation answers 409
  `cursor-generation-changed` (`src/web.rs:1686-1695`; browser
  `app.js:472-483`).

Natural attachment points for a faster second source:

- A sibling holder next to `source` in `WebState` (`src/web.rs:67-76`) with
  its own `watch` channel modeled on `LiveSource::notify`, spawned next to
  `src/web.rs:130-136`.
- A new route in `build_router` (`src/web.rs:211-247`) plus either a second
  browser poll loop or an extra field on `WatchProjection`
  (`src/web.rs:543-548`).
- Do NOT ride the generation counter: `publish()` increments `current`,
  resets `change_pending`, and evicts (`follow.rs:233-271`); "Snapshot
  generation" is a defined domain term (`CONTEXT.md:239-241`); buffer-state
  ticks advancing generations would redefine it and invalidate open cursors.

## Q3 — Inspection graph structure and cost of a VPID-keyed overlay

- `SessionData` (`src/inspection.rs:998-1032`) holds `fast_summary` plus
  VPID-keyed BTreeMaps (`page_overrides`, `deep_pages`, `file_allocations`,
  `record_interpretations`, `interpretation_failures`, …).
  `Inspection`/`GraphView` are `Arc<SessionData>` (`src/inspection.rs:1246-1254`);
  `view()` refuses any revision but the exact one (`:1971-1982`).
- Views are assembled on read by joining those maps: `sector()`
  (`:2069-2096`), `page_from_record` (`:2098-2110`), association joins
  (`:2114-2178`). That join is the single place a graph-resident overlay
  would attach — but it should not, see below.
- Enrichment = clone `SessionData`, insert evidence, bump revision, new `Arc`
  (`enrich_page` `:2274`+ and siblings); the web layer republishes as a
  revision of the same generation (`follow.rs:278-294`,
  `src/web.rs:1546-1606`).
- Evidence is defined against volume byte ranges
  (`src/model.rs:349-522`). Volatile buffer state has no byte range and no
  rule id, so it cannot be `Evidence`.
- An out-of-graph overlay is cheap mechanically: `GraphView` is immutable +
  `Clone`; an overlay beside it in `WebState` needs no `SessionData` change
  and no revision bump. Projections allow additive fields at
  `SCHEMA_VERSION = 1` (`src/projection.rs:16-17`, `:174-176`, `:216-218`).
- Friction: `page_projection`/`sector_projection` are shared by CLI, TUI,
  HTML export and JSON (`src/projection.rs:765-773`, `:850-889`; consumers
  `src/tui.rs:626`, `src/cli.rs:1160-1175`, `src/export.rs:104-134`).
  A web-side sibling field in `PageResourceProjection`/the sectors response
  (`src/web.rs:1088-1094`, `:679-683`) or a standalone endpoint avoids
  forcing every adapter to carry an `Option`.
- Doc-level tension: `CONTEXT.md:11-13` — an adapter "never invents
  adapter-specific storage facts"; `CONTEXT.md:15-17` scopes the Projection
  workspace. Both point at the overlay needing a new domain term declaring it
  not a storage fact, rather than smuggling it into `PageProjection`.

## Q4 — TUI parity contract

- Definition: "Terminal interaction parity … preserves the web viewer's …
  semantic visual distinctions, including page occupancy and structural
  distribution" (`CONTEXT.md:23-25`).
- The semantic-rendering ticket fixes a closed seven-channel page strip
  `[focus][physical type × 2][allocation][occupancy][finding][selection]`
  that "never removes or merges these channels"
  (`.scratch/volmap-tui-web-parity/issues/06-define-semantic-terminal-rendering.md:26-38`).
- Verification contract: parity facts are the same typed facts for one exact
  immutable revision
  (`.scratch/volmap-tui-web-parity/issues/08-define-parity-verification-contract.md:35-40`);
  parity matrix (`:76`), 36 renderer golden pairs (`:157-165`), Playwright
  suite (`:320-335`).
- Precedent for scoping a semantic out of parity: record-value parity
  exclusion (`…/08-…md:369`).
- Implementation status: no `AtlasMachine`/`AtlasRenderer` exists in `src/`;
  today's TUI shows allocation + findings but no occupancy
  (`src/tui.rs:622-649`) — the TUI already lags a web semantic.

Net: a web-only overlay breaks no executing test today, but collides with the
parity definition's "semantic visual distinctions" clause, the fixed
seven-channel strip, and the exact-revision-facts premise. Needs an explicit
decision, not a quiet web-only addition.

## Q5 — Contract statements this feature touches

- `README.md:3-8` read-only offline inspector; `README.md:171` never writes;
  `README.md:162-178` safety and scope ("observed disk state, not
  transactional committed state", `:167-170`).
- `CONTEXT.md:263-265` **Observed disk state** — live follow "does not make
  the viewer a transaction-visibility tool". Buffer state is engine memory —
  outside this term; needs a new term, not a widened one.
- `CONTEXT.md:271-273` **Standalone executable** — "no runtime dependency on
  glibc, CUBRID libraries, installation assets, network services, or
  separately installed web assets". A cub_server inspector client is the most
  direct contract collision found; the overlay must be strictly optional and
  degradable.
- `CONTEXT.md:231-233` **Live inspection session** — unauthenticated HTTP;
  remote exposure requires explicit IPv4 wildcard.
- Security posture: loopback default, deliberately unauthenticated
  (`README.md:92-142`); `validate_listener` (`src/web.rs:1745-1752`), Host
  pinning + method allowlist + POST Origin checks (`src/web.rs:292-375`),
  CSP `default-src 'none'; connect-src 'self'` (`src/web.rs:377-400`),
  `access: "unauthenticated-http"` in `/api/v1/session`
  (`src/web.rs:522-535`), tests `src/web.rs:1824-1830`,
  `src/web/assets.rs:112-120`.
- ADR-0001 (`docs/adr/0001-explicit-target-disclosure.md:32-33`): "every
  adapter projects the same committed facts" — strongest documented statement
  against an adapter-only semantic layer; an exemption ADR is needed.
- Supply chain: pinned `=` deps and `unsafe_code = "forbid"`
  (`Cargo.toml:17-32`, `:41-42`), `provenance.toml` decoder table,
  notices/SBOM recipe (`justfile:87`), cost model stated in
  `.scratch/live-follow/SPEC.md:129-130`.

## Q6 — Existing live-server interaction

None. No hits for `cub_server`, `pgbuf`, "page buffer", gRPC, protobuf,
tonic, or prost in `src/`, `tests/`, `README.md`, `CONTEXT.md`. Only design
docs: `docs/live-page-buffer-inspection.md` and the live-follow spec's
explicit non-goal (`.scratch/live-follow/SPEC.md:158-161`). All live
interaction today is file polling + positional reads (`src/source.rs:543-560`,
`src/follow.rs:319-415`).

Dependency surface: `axum` features `http1, json, query, tokio` (no HTTP
client); `tokio` features `net, rt-multi-thread, signal, sync, time`
(`Cargo.toml:20`, `:31`). Tokio's `net` includes Unix-domain sockets, so a
local UDS client needs no new crate; any framed wire format beyond
`serde_json` would.

## Integration points and risks (summary)

Points, in order of fit: (1) `WebState` sibling holder + own watch channel;
(2) new `/api/v1/...` overlay resource and/or `WatchProjection` field;
(3) response join sites `src/web.rs:622-684`, `:1088-1094`;
(4) browser render hook `applyPageFill` (`app.js:497-506`), cells
`app.js:817-823`/`:883-908`, legend `app.js:420`, styles `app.css:247-284`;
(5) follow plumbing for cadence/pause (`app.js:248-282`).

Risks: contract wording (Observed disk state; Standalone executable);
"adapters never invent facts" (ADR-0001) needs a documented exemption;
parity (closed seven-channel strip; exact-revision facts); generation
coupling (never advance generations for overlay ticks); snapshot/overlay
skew (report overlay observation time in envelopes); security
(unauthenticated port exposing engine internals; CSP `connect-src 'self'`);
release gates (pinned deps, SBOM/notices, provenance); export/asset test
drag (sha256-pinned export CSP `src/export.rs:243,246`; asset marker tests
`src/web/assets.rs:64-120`).
