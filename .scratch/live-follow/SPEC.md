# Live volume follow

## Problem

`volmap serve` pins one immutable database snapshot for the life of the
process. The snapshot identity folds every volume's file stamp into a content
hash (`Inspection::open`), every deep operation re-checks the stamps, and a
mismatch publishes a terminal `SnapshotValidity::Invalidated` revision. The web
adapter then retroactively marks *every* retained revision invalidated
(`projected_overview`), and browser URLs hard-pin `/s/{snapshot}/r/{revision}/`,
so `revision_view` answers `revision-not-found`.

The practical consequence: one `csql` statement against the inspected database
poisons the whole viewer session and breaks every open link. Volmap's offline
contract is correct for forensics on a stopped database or a copy, but it is the
wrong contract for watching a running database, which is how the tool is
actually used day to day.

## Decision

`serve` gains **live follow**: it watches the input, re-reads it when it
changes, and publishes a new **snapshot generation**. The offline immutable
contract is preserved everywhere else and remains available in `serve` behind
`--no-follow`.

Three product decisions, confirmed before implementation:

1. **Live-only entity URLs.** Web paths name an entity, not a generation.
   Generation, revision, and validity stay in every JSON envelope so nothing is
   misreported. `export html` remains the way to freeze a view.
2. **Follow is the default for `serve`**, and only for `serve`. `summary`,
   `map`, `inspect`, `export html`, and `tui` keep the immutable contract.
3. **Auto-refresh with pause.** The browser re-renders at the new generation
   preserving drill level and scroll, and a header chip offers a pause that
   freezes the view on its current generation.

## Vocabulary

Added to `CONTEXT.md`; summarised here.

- **Source mode** — `immutable` or `live`. Selects the consequence of an input
  change, not the reading itself.
- **Snapshot generation** — one complete fast scan of a live input, numbered
  monotonically within a live inspection session, carrying its own
  database-snapshot identity and its own inspection-revision chain. Generations
  *replace* one another; they are not revisions of one another.
- **Input fingerprint manifest** — the ordered declared volume identities and
  their file stamps observed for one generation, including volume-set
  membership so an added or removed volume is a change.
- **Torn generation** — a generation whose manifest changed during its own
  scan. Usable evidence labelled internally inconsistent; not an invalidated
  snapshot.
- **Superseded generation** — a published generation whose manifest no longer
  matches the input. Its facts stay exactly as observed and stay queryable
  while retained.
- **Live follow** — the watcher-plus-re-read behaviour above.
- **Generation retention window** — the bounded count of recent generations
  kept addressable so in-flight work finishes on the generation it started on.

## Source-change semantics

| Observation | Immutable mode | Live mode |
| --- | --- | --- |
| Manifest changed during the open scan | `Invalidated`, `snapshot.modified` (fatal) | `Torn`, `snapshot.torn_read` (warning); a re-read is scheduled |
| Manifest changed before an enrichment | `Invalidated`, terminal | enrichment proceeds; result is `Superseded` with `snapshot.source_advanced` (warning); a re-read is scheduled |
| Watcher observes a manifest change | not applicable | debounce, re-read, publish generation N+1 |

A superseded or torn generation is never retroactively rewritten. The web
adapter's blanket `apply_terminal_invalidation` is confined to immutable mode.

## Re-read policy

The watcher polls the input fingerprint on `poll_interval`. Fingerprinting
stats volume paths and re-reads the small `_vinf`/`databases.txt` manifest; it
does not read pages. A transient fingerprint error is "unknown, retry", never a
crash.

The scan trigger is a pure function so it can be tested without timers:

```text
should_rescan =
      change_pending
  and (since_last_change >= quiet_period or since_first_change >= max_defer)
  and since_last_scan    >= max(min_idle, last_scan_duration)
```

`max(min_idle, last_scan_duration)` is the load governor: a scan that takes
three seconds cannot re-run more than about every three seconds, so following a
large volume stays under a 50% duty cycle without a tuning knob per database.

Defaults: `poll_interval` 500 ms, `quiet_period` 300 ms, `max_defer` 3 s,
`min_idle` 250 ms, retention 4 generations.

A continuously written volume never goes quiet, so `max_defer` bounds staleness
and the resulting generation is usually `Torn` — which is the honest answer, and
the label says so.

## HTTP surface

Browser routes, all serving the shell:

```text
/                          /page/{vol}/{page}
/volume/{vol}              /slot/{vol}/{page}/{slot}
/sector/{vol}/{sector}     /oos/{vol}/{page}/{slot}
```

API:

```text
GET  /api/v1/session
GET  /api/v1/licenses
GET  /api/v1/live/watch?generation=N
GET  /api/v1/overview      GET /api/v1/volumes      GET /api/v1/sectors/{vol}
GET  /api/v1/relationships GET /api/v1/diagnostics  GET /api/v1/coverage
GET  /api/v1/file/{vol}/{file}     GET /api/v1/sector/{vol}/{sector}
GET  /api/v1/page/{vol}/{page}     GET /api/v1/slot/{vol}/{page}/{slot}
GET  /api/v1/oos/{vol}/{page}/{slot}
POST /api/v1/enrichments
```

Every envelope's `snapshot` object gains `generation` and
`observed_at_unix_seconds`, and `validity` widens to
`valid | torn | superseded | invalidated`.

`/api/v1/live/watch` long-polls: it returns at once when the current generation
differs from `generation`, otherwise it waits on a `tokio::sync::watch` channel
for up to 25 seconds and reports whether the generation advanced. Long-poll was
chosen over SSE because it needs no new dependency — the release graph is
pinned and every addition costs an SBOM, notices, and licence-audit cycle.

The revision-URL and `/api/v1/jobs/{job}` machinery is removed. It existed only
to hand the browser a new pinned URL after enrichment; with live URLs the
enrichment response carries the resulting entity path directly.

### Cursors

Progressive mosaic cursors currently MAC the snapshot id *and* revision, so any
enrichment mid-load silently invalidates an open cursor. The cursor payload
becomes `generation || offset`, MAC'd over `kind || payload`. Authenticity is
unchanged, enrichment no longer breaks a load, and a cursor from an older
generation is distinguishable from a forged one — it answers
`cursor-generation-changed` (409) so the browser restarts the mosaic instead of
reporting a bad request.

## Browser behaviour

- Bootstrap reads `/api/v1/session`, learns the generation and whether follow
  is on, then runs a long-poll loop.
- On advance: adopt the new generation, drop the sector cache, and re-render
  the current drill level with history mode `none`, restoring scroll position.
- Header chip `live · gen 7 · 2s ago` with a Pause control. Paused stops the
  loop and shows `paused at gen 7 · newer: gen 9`; resuming refreshes.
- Enrichment keeps the live URL; it no longer pushes a revision into history.

## Out of scope

- The TUI keeps the immutable contract.
- `export html` still freezes exactly one revision of one generation.
- No page-buffer cooperation with a running `cub_server`; that remains the
  separate design note in `docs/live-page-buffer-inspection.md`.
- No incremental re-scan. CUBRID does not tell an outside reader which pages
  changed, and establishing that from the outside costs the same envelope read
  as the full fast scan, so the governor bounds cost instead.

## Acceptance

1. With `serve` following, writing to an inspected volume advances the
   generation and every open browser view re-renders without a failed fetch.
2. A live URL copied at generation 3 still resolves at generation 40.
3. `serve --no-follow` reproduces today's immutable behaviour exactly,
   including the terminal invalidated snapshot.
4. A generation whose manifest changed mid-scan reports `torn`, not `fatal`,
   and schedules another scan.
5. A cursor issued in an older generation answers `cursor-generation-changed`,
   never `invalid-cursor`.
6. `just verify` passes: `fmt-check`, `test-debug`, `lint`, `elf-check-release`.
