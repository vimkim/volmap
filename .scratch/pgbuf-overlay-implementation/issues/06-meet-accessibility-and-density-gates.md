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

2026-09-10 — Resumed allocation investigation identifies both automation selector
allocations and dense enable/mode render allocations. Source-mapped native-control
profiles retain about 24.8 MiB setup allocation after selector work is removed;
the corresponding RSS diagnostic still fails. Both original browser cases were
rerun and remain non-passing. See [allocation findings and repair targets](../verification/06-allocation-profile/README.md).
No production change or gate relaxation was made. Memory diagnosis can proceed
independently of the outstanding manual reviews; the external prerequisites do
not block investigating these identified render paths.

2026-09-10 — Fix-phase checkpoint `0a2f5ea` reduces the demonstrated render
allocations and fixes a tested transient whole-map failure projection while the
first capture is pending behind cached unavailable metadata. The focused test
was red before the guard and passes through actual failure and recovery in both
browsers. Full `just verify` passes (73 frontend unit tests, 41 browser passes,
existing one Firefox skip, Rust/release checks).

The original fresh-release 12,288-cell / 40-input density command still fails the
unchanged 32 MiB gate overall in both browsers; passing latency and lower sampled
JavaScript allocation do not establish memory acceptance. No checklist item is
closed. See [the repair evidence](../verification/06-memory-fix/README.md) for
retained/rejected experiments, source-mapped profiles, raw results and remaining
native-memory investigation. Manual and reference-host prerequisites remain
outstanding but do not block further memory repair.

2026-09-10 — Native-allocation diagnosis on repaired consumer `0a2f5ea` isolates
browser-specific causes before further production changes. Firefox's pending
refresh-button opacity triggers about 16 MiB of software WebRender storage;
removing only that opacity prevents the allocation, and applying opacity to an
existing button with observations disabled recreates it. The full opacity-only
CSS diagnostic passes Firefox at 11.52/12.33 MiB, but Chromium still fails at
45.54/38.44 MiB. This diagnostic removes a visual disabled cue and is not a
finished accessible repair or acceptance evidence.

Chromium reverse controls independently show +10.25 MiB tile storage from moving
the map by the legend's height and +7.50 MiB from applying its LRU colors, without
observations or corresponding V8 growth. Native allocations and ownership aliases
are not substituted for the unchanged summed-RSS gate. See [native causes, raw
profiles and next repair targets](../verification/06-native-allocation/README.md).
No production files, thresholds, browser scope or checklist state changed.

2026-09-10 — Native repair replaces pending-button opacity with an opaque dashed
disabled appearance and relocates the Volume legend beside observation controls.
Both regression cases were red before their respective changes and now pass in
both browsers. The map no longer moves when observations are enabled or LRU mode
is selected. The full release density run retains all 12,288 cells and 40 inputs:
Chromium 29.62/32.95 MiB, Firefox 23.73/23.45 MiB, worst p95 53.4/58.0 ms.
**Chromium LRU still fails 32 MiB; ticket acceptance remains non-passing.**
The optional density port supports a fresh server alongside an existing preview;
it does not change measurement or server-reuse rules. See [native repair evidence](../verification/06-native-repair/README.md).
Further Chromium raster investigation and the existing manual/reference-host
prerequisites remain open. No checklist item is closed.

2026-09-10 — Follow-up on committed repair `978efae` still fails the original
full workload: Chromium 26.76/41.48 MiB, Firefox 16.49/23.48 MiB; worst p95
69.2/64.0 ms. Native reverse controls separate LRU recoloring from additional
glyph paint and observation-update work during navigation. Tested paint/layout
containment, normal-layout glyph placement and a nonresident pseudo-element did
not establish a safe improvement, so no further production change was adopted.
See [the raw profiles and rejected probes](../verification/06-raster-followup/README.md).
The 32 MiB gate, both-browser workload and human/reference-host prerequisites
remain unchanged; the earlier 32.95 MiB Chromium LRU result was not a stable near-pass.
