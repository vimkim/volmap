# 08: Establish release evidence

**What to build:** A release reviewer can reproduce the accepted performance matrix, inspect a complete exact-commit gate manifest and use operator documentation to decide whether this producer is ready for delivery.

**Blocked by:** 07 — Port and verify on develop; external prerequisites: a recorded dedicated measurement host, companion Volmap browser performance/accessibility evidence (including companion tickets 05–06), and upstream acceptance of the new socket for final readiness.

**Status:** ready-for-agent

- [ ] Provide a reproducible measurement driver and record hardware, software/build revisions, producer/consumer/corpus/testcase identities, datasets, page size, concurrency, seeds and raw samples. Run release builds on a pinned dedicated host; do not invent hosted CI or substitute local convenience commands for project verification instructions.
- [ ] Exercise 512 MiB, 1 GiB and 4 GiB pools at 16 KiB/page under idle, read-heavy, write/flush-heavy and churn workloads, with 1/8/32 tabs plus stalls, viewport rotation, pause/hidden, clock changes and restart.
- [ ] Run ten paired runs per performance case, each with 60 s warmup and at least 5 min measurement, extending to at least 100,000 transactions where applicable. Match workload, database state, concurrency and build except inspector activity. The 95% confidence upper bound must satisfy at most 2% throughput loss and 5% transaction p99 increase; uncertain or individually failing cases do not pass.
- [ ] Measure an unmodified build against parameter-off and enabled/no-demand separately from active observation. At default cadence require producer CPU at most 20% of one logical core and incremental peak RSS at most 16 MiB. Report CPU-time/wall-time, allocator peaks and RSS independently.
- [ ] For idle/read-heavy 1 GiB reference require at least 99% complete scans and p95 refresh demand-to-validated-publication at most 250 ms. Measure completeness separately from latency; oversized pools require truthful partial coverage rather than reference completeness.
- [ ] Join companion evidence for broker CPU at most 50% of one core, incremental RSS at most 192 MiB, browser at most 32 MiB per tab and accounted broker allocation at most 128 MiB. Require cached HTTP p95 at most 25 ms and simultaneous disk-inspection p95 regression at most 5%. Exercise retained/in-flight/response allocations together.
- [ ] Collect actual 10,000-cell rendered-density evidence in Chromium and Firefox with p95 input-to-visible update at most 100 ms, semantic/non-color/keyboard checks, high contrast, reduced motion and manual screen-reader/visual review. Identify reviewer, browser/assistive technology and results; screenshots alone do not prove accessibility.
- [ ] Complete a gate manifest mapping every producer specification invariant and budget to its owner/test, exact commits/corpus hash, build mode, external testcase revision, fixture preconditions, command, nonzero executed counts, result and raw artifacts. Include both format-aligned full integration and develop producer debug/release evidence. Code/corpus changes invalidate affected entries.
- [ ] Write operator documentation for startup enablement, local socket configuration, same-account authorization, activation failures, lifecycle, coverage, observation age and limitations. Distinguish sampled state from atomic snapshots, page-image correspondence, transaction visibility, durability or flush-event history. Exclude page capture, AOUT, event hooks, native LRU telemetry and runtime TUI/export scope.
- [ ] Record upstream socket acceptance and all external delivery evidence without claiming a local ticket grants upstream approval. Missing, skipped, failing or inconclusive required evidence keeps readiness open. Tuning within accepted limits is allowed; weakening limits requires an explicit design revision. This ticket joins companion evidence and does not wait on the companion final-readiness ticket closing.
- [ ] Prepare concrete reviewable release material; external JIRA/PR publication remains a separate authorized workflow. Completing the documentation alone cannot close this ticket while quantitative or manual gates remain open.

## Source and scope

Implements the approved CUBRID producer specification for CBRD-27398 in this feature tracker. Read its full contract and linked decision authority before implementation. The public protocol is the main test boundary; focused deterministic scan/serializer checks supplement real I/O. This ticket does not add resident-page capture, AOUT or event history, native LRU telemetry, or Volmap UI work.

## Comments

2026-09-09: Published after the user approved the eight-ticket breakdown and blocking edges. Ready-for-agent describes triage readiness; blockers and acceptance criteria remain unsatisfied until evidenced.
