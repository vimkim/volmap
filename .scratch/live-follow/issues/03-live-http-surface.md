# 03 — Live entity URLs and the watch endpoint

Blocked by: 01, 02. Blocks: 04

- Replace `/s/{snapshot}/r/{revision}/...` with live entity paths for both the
  shell routes and `/api/v1`, per SPEC.
- Resolve every handler against the current generation's latest revision.
- Add `GET /api/v1/live/watch?generation=N` long-poll (25 s cap).
- Envelope `snapshot` gains `generation` and `observed_at_unix_seconds`;
  `validity` widens to four values.
- Cursor payload becomes `generation || offset`, MAC over `kind || payload`;
  an older generation answers `cursor-generation-changed` (409).
- Remove `/api/v1/jobs/{job}`; `POST /api/v1/enrichments` answers 200 with the
  resulting live entity path.
- Confine `apply_terminal_invalidation` to immutable mode.
