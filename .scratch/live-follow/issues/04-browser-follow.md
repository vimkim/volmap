# 04 — Browser follows generations

Blocked by: 03

- `routes.js`: live entity grammar, no snapshot or revision segments.
- `app.js`: long-poll follow loop; on advance adopt the generation, clear the
  sector cache, re-render the current drill level with history mode `none`, and
  restore scroll.
- Header chip `live · gen N · Ns ago` plus Pause/Resume; paused shows
  `paused at gen N · newer: gen M`.
- The chip reports **two** times, not one. `Ns ago` is when Volmap last read
  the input; alongside it show `disk HH:MM` from the envelope's
  `input_modified_unix_seconds`. A reader who has just committed a change and
  cannot see it needs to distinguish "Volmap has not looked recently" from
  "the engine has not flushed yet", and only the second time answers that.
  Confirmed in use: dropping a table left it visible for minutes, because the
  commit was durable in the log while the data volumes were untouched.
- Rename the existing `loadGeneration`/`routeGeneration` load-cancellation
  locals so `generation` means the source generation throughout.
- Restart the mosaic load on `cursor-generation-changed`.
- Enrichment stops pushing a revision URL into history.
- On advance, a slot or OOS drill level that only became addressable through an
  enrichment must not be re-rendered into `entity-not-found`. Enrichment does
  not carry across generations — `CONTEXT.md` states this outright — so the
  browser re-issues the enrichment that opened the level and then re-renders,
  falling back to the parent level only if that fails. Reproduced against a
  real database: enrich a page, write to the volume, and the slot URL that
  answered a moment earlier answers 404.
