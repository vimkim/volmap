# 06: Meet accessibility and dense-view interaction gates

**What to build:** Keyboard and assistive-technology users can operate the complete live overlay without disrupted focus or noisy updates, and operators can navigate a genuinely dense view responsively in both supported browsers. This is an end-to-end product acceptance slice, including bounded improvements needed to pass its gates.

**Blocked by:** 03 — Handle pause, resume, failures, and producer restarts; 05 — Render the heatmap and LRU topology; named manual screen-reader/visual reviews and reference-host/peak-memory qualification.

**Status:** ready-for-human

- [x] Exercise integrated lifecycle and visible-scope rendering at Volume and Sector scales, including state-mark/LRU modes, exact-index detail and fresh/stale/paused/expired/absent/refused states. Assert semantic roles, accessible names, non-color limitations and stable focus through refresh and navigation.
- [ ] Prove keyboard operation, high-contrast readability, reduced-motion behavior and quiet age/poll updates. Record manual screen-reader and visual reviews with reviewer and assistive technology; screenshots alone cannot establish accessibility.
- [ ] Render 10,000 actual page cells in the measured density case, recording rendered count, viewport, browser versions, reference host and raw interaction samples. A 10,000-entry off-screen model is not sufficient evidence.
- [ ] Measure p95 input-to-visible update at no more than 100 ms independently in Chromium and Firefox, alongside matched-disabled browser memory measurements against the 32 MiB incremental peak RSS per-tab limit. Do not average browsers together or replace visible updates with model-only timing.
- [x] Include viewport churn, partial coverage, shared demand, pause/resume, hidden tabs and restart/expiry in the integrated browser checks. Any optimization preserves selected priority, explicit rotation, bounded requests, original age and late-response revocation.
- [x] Run through established local/release browser tooling without inventing hosted CI. Retain exact consumer commit, corpus hash, environment, build, commands, named cases, executed counts and raw artifacts for the delivery manifest. Missing manual review or an inconclusive/failed browser gate remains non-passing.
- [x] Make only contract-preserving improvements needed to satisfy this user journey; do not weaken density, timing, memory or accessibility thresholds or introduce a frontend rewrite. Runtime evidence remains outside disk inspection facts, TUI and exports.


## Comments

2026-09-10 — Automated implementation and review completed, but **acceptance is
non-passing**. Eight new accessibility cases pass in Chromium and Firefox;
`just verify` passes with 306 Rust, 73 frontend and 39 browser cases (existing
three Rust ignores / one Firefox skip). Grid rows, quiet age updates, visible
navigation focus, deferred disclosure tables and bounded render reuse are shipped
in consumer `8810d04`.

The real-server density case lays out 12,288 cells (1,984 in the initial
1920×1080 viewport). State/LRU input timing passes locally: worst-mode p95
68.7 ms Chromium, 55 ms Firefox. **Both memory diagnostics fail**: sampled
increments range 33.57–55.26 MiB against the unchanged 32 MiB ceiling.
Manual screen-reader and visual reviews, with named reviewer/assistive technology,
remain missing; a qualified reference host/peak-memory measurement is also needed.
`ready-for-human` identifies that evidence/measurement handoff, not completion or
permission to weaken a gate. Further memory repair may be needed after qualified
measurement. Tickets 07–08 do not inherit a passing prerequisite.

See [the verification record](../verification/06-accessibility-density.md),
[delivery manifest](../verification/06/manifest.json), and preserved raw failure
history. Reproduce diagnostics with `just vite::frontend-density`.

2026-09-10 — Completion audit: reopened the density-evidence checklist item
because its reference-host requirement remains unresolved. The 12,288-cell
rendered count and raw samples are verified local evidence, but do not establish
the entire item. Follow-up diagnosis in `9df2c31` reproduced the memory failure;
neither equivalent CSS selectors nor diagnostic-only forced GC established a
passing gate or a specific production fix. Manual review and reference-host
inputs remain the external prerequisites for advancing acceptance.
