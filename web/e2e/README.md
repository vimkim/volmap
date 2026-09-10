# Browser assertion boundary

Blocking browser tests exercise the embedded application through the real Rust
HTTP server. Assertions use URLs, response security headers, semantic roles,
accessible names, and rendered facts. CSS classes and screenshots are review
aids, not acceptance authorities.

## Overlay accessibility and density

`overlay-accessibility.spec.ts` checks row/gridcell ownership, keyboard activation,
focus after navigation, compatible focus during polling, quiet live regions and
forced-color focus in Chromium and Firefox. `observations.spec.ts` supplies the
integrated lifecycle, partial coverage, shared demand, viewport rotation,
restart/expiry and exact LRU detail cases. These are automated semantic checks;
they do not establish manual screen-reader or visual acceptance.

Run the separate release-build density diagnostic with the pinned tooling:

```sh
just vite::frontend-density
```

DOM tracing is disabled for this diagnostic because recording large DOM snapshots
perturbs browser timing and memory; raw JSON samples remain mandatory.
It uses the real Rust server on port 41742, a deterministic `--dense` smoke
fixture (192 sectors / 12,288 pages), the scripted producer corpus, and normal
collection pagination. It counts laid-out page cells and separately records
those inside the 1920×1080 viewport. It does not substitute model entries for DOM
cells. Each browser gets two order-alternated enabled/disabled pairs with a fresh,
one-tab browser process tree. The same 40 keyboard focus changes run in each arm; the enabled cases cover
state marks and LRU separately, and neither case may exceed the timing limit.
The keydown-to-second-animation-frame interval is a conservative visible-focus
latency bound; only a changed, on-screen, outlined focus target counts. Per-browser
p95 must be ≤100 ms. RSS is sampled every 50 ms across the dedicated browser tree;
per-pair incremental sampled peaks must be ≤32 MiB. Summed process RSS counts
shared mappings repeatedly and a sampled peak can miss shorter peaks: this local
probe is diagnostic, not qualified peak-memory or full release acceptance.

Raw JSON attachments preserve every interaction and RSS sample, counts, viewport,
browser version, host, commit, working-diff hash and producer corpus hash. Archive
attachments before another Playwright invocation clears `test-results`. Retain
an exact committed consumer rerun for delivery. A build, assertion or sampling
failure is non-passing, and a successful diagnostic never closes missing manual
or reference-host evidence. No hosted CI is configured by this workflow.

Manual acceptance must record reviewer, date, OS/browser version, assistive
technology/version, named case, outcome and notes for both scales: keyboard-only
navigation; state/LRU interpretation without color; exact-index detail; fresh,
stale, paused, expired, absent and refused states; focus on refresh/navigation;
quiet polling; high contrast; reduced motion. Screen-reader and visual review
are separate required records. Screenshots are supporting evidence only.
