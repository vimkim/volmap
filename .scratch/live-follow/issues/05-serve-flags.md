# 05 — `serve --follow` / `--no-follow`

Blocked by: 02

- `ServeCommand` gains `--follow` / `--no-follow` (default follow on) and
  `--follow-interval-ms` (default 500) and `--follow-retain` (default 4).
- `serve` passes `SourceMode::Live` when following, `Immutable` otherwise.
- No other command changes behaviour.
- `serve` prints the follow state next to the listener URLs.
