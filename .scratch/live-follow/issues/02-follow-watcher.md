# 02 — Follow watcher and generation store

Blocked by: 01. Blocks: 03

- Pure `should_rescan(RescanInputs, FollowConfig) -> bool` per SPEC, with
  `Duration` inputs so it is testable without timers.
- `GenerationStore`: newest-N generations, each with number, base view,
  revision map, fingerprint, validity, `observed_at`, and scan duration.
  Publishing evicts beyond the retention window.
- `tokio::sync::watch::Sender<u64>` carrying the current generation number.
- Watcher task: poll the fingerprint on `poll_interval`, apply `should_rescan`,
  run `Inspection::open` on a blocking thread, publish. Fingerprint errors are
  "unknown, retry".

Done when: `should_rescan` is unit tested across quiet-period, max-defer, and
governor cases, and the store's eviction keeps the current generation.
