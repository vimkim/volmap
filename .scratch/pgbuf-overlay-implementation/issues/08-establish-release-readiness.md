# 08: Establish performance evidence and release readiness

**What to build:** A release reviewer can decide whether the complete overlay is safe and responsive from exact-commit evidence, and an operator can understand how to enable it and interpret its limitations. Simulated or partial success cannot stand in for required release gates.

**Blocked by:** 06 — named manual screen-reader and visual reviews; dedicated-host performance campaign and final delivery evidence. Ticket 07 functional verification is complete; see 07-develop-disk/completion-audit.md.

**Status:** ready-for-agent

- [ ] Run release-build measurements on a recorded dedicated host for 512 MiB, 1 GiB and 4 GiB pools at 16 KiB/page; idle, read-heavy, write/flush-heavy and churn workloads; 1/8/32 tabs; and stall, rotation, pause/hidden, clock-change and restart scenarios. Include consumer, producer and browser behavior rather than isolated empty scans.
- [ ] For each performance case collect ten paired runs, each with 60 seconds warmup and at least five minutes measurement, extending to at least 100,000 transactions where applicable. Paired runs retain identical workload, state, concurrency and build except inspector activity. Preserve raw samples and the confidence method; do not average away failing cases.
- [ ] The 95% confidence upper bound must meet at most 2% throughput loss and 5% transaction p99 increase. Measure parameter-off versus an unmodified build and enabled-without-demand separately. Inconclusive results are not passing evidence.
- [ ] For idle/read-heavy 1 GiB reference cases, require at least 99% complete scans. Require p95 refresh demand-to-validated-publication no more than 250 ms, cached HTTP request-to-completed-response p95 no more than 25 ms, and concurrent disk-inspection p95 regression no more than 5%. Record completeness separately so mostly empty fast scans cannot pass. Larger pools require truthful partial coverage, not the reference completeness percentage or expanded resources.
- [ ] At default cadence, require producer CPU at most 20% and broker CPU at most 50% of one logical core. Incremental peak RSS must stay within producer 16 MiB, broker 192 MiB and browser 64 MiB per tab relative to matched disabled workloads/tab counts. Report CPU-time/wall-time and allocator accounting separately; RSS allowances do not relax 48 MiB decoded-scan or 128 MiB total broker allocation limits.
- [ ] Include ticket 06's actual 10,000-rendered-page Chromium and Firefox evidence with p95 input-to-visible update at most 100 ms per browser, manual screen-reader/visual review and lifecycle/coverage accessibility checks. Include ticket 07's real-engine debug/release and develop-versus-format-aligned evidence, not just offline corpus results.
- [ ] Maintain a delivery gate manifest mapping every invariant and budget to owner/test, exact producer/consumer commits, corpus revision/hash, build mode, environment, testcase revision, preconditions, command, executed count, result and raw artifacts. Manual reviews identify reviewer and assistive technology. Every required missing, skipped, inconclusive or failing result keeps release readiness open; affected evidence reruns after code/corpus changes.
- [ ] Use established local/release verification tooling. Any contract-preserving tuning needed to meet a gate is reverified; lowering a threshold requires an explicit design revision. Neither this ticket nor a green subset invents hosted CI or claims unexecuted external test results.
- [x] Complete operator and maintainer documentation using the accepted glossary and loopback/web-only ADR: explicit enablement/socket configuration, SSH forwarding, verification/refusal, coverage versus completeness, conservative age/expiry, pause/resume, resource limits and producer/corpus compatibility. Preserve existing domain decisions rather than silently redefining them.
- [x] Explain that observed disk state and page-buffer observation are independent; neither VPID matching, sequence, recency nor sampled flushing proves currentness, image correspondence, commits or durability. Preserve all exclusions: no resident-page inspection prerequisite, AOUT history, transition events, TUI/export/inspection-graph runtime state, new kernel-cache features, discovery/public transport, engine writes or hot-path instrumentation.
- [ ] Hand off the manifest and documentation without treating the ready-for-agent ticket status as release approval. External JIRA upload, commits/pushes or PR publication require their own authorized workflow; do not alter the parent specification or completed planning map to imply delivery.


## Comments

2026-09-11 — Preparation started from consumer `08ed947`, after reading 06's
unfilled named manual-review record and 07-develop-disk's completion audit.
Ticket 07 remains functionally complete. Ticket 06's local density pass is
historical evidence; two of its six source fingerprints have changed, so affected
candidate evidence requires reruns. The 07 runtime records retain dirty-base,
source and binary provenance rather than being relabelled clean-commit runs.

Prepared the [delivery index and handoff](../verification/08-release-readiness/README.md),
[measurement protocol](../../../docs/runtime-overlay-release.md), and expanded
[operator documentation](../../../docs/runtime-overlay.md). The manifest lists
unexecuted gates and missing inputs explicitly; final per-invariant delivery
acceptance is still open. No manual review or performance case ran, no threshold
changed, and no readiness approval is claimed. The accepted ADR 0006 now permits
explicit IPv4 listeners as well as loopback; the parent specification and planning
map are preserved. Work item 140 tracks the remaining campaign.

Preparation validation: `just verify` passes (74 frontend unit cases, 65 browser
passes and one existing Firefox parity skip, plus Rust/Clippy/static-musl gates).
Independent Standards and Spec reviews of `08ed947...86d7f35` report zero
findings for the preparation scope; see [verification and reviews](../verification/08-release-readiness/verification.md).
Only the operator/limitation documentation items are closed. Measurement,
manual accessibility, candidate reruns and final named invariant mapping remain
open. No release approval or external publication follows from this commit.

2026-09-11 — Current-source density rerun on `cab4a9b` provides new non-passing
evidence: Chromium LRU incremental sampled RSS is 34.54 MiB against 32 MiB;
Firefox and both browsers' timing pass. Two predetermined Chromium-only repeats
pass at 27.26/26.16 MiB LRU increments, establishing variability rather than a
repair or a specific regression cause. All four browser executions retain the
original 12,288-cell / 40-input-per-arm workload. See [raw attempts and diagnosis
boundary](../verification/08-candidate-density/README.md). The candidate-density
gate is now explicitly failed; historical 06 acceptance is preserved. No runtime
code or threshold changed. Manual and dedicated-host gates remain open.

2026-09-11 — Added a [named functional evidence ledger](../verification/08-contract-ledger/README.md)
for all 50 checklist entries in tickets 01–05: 81 test/source references, source
fingerprints, execution contexts and explicit limits instead of aggregate-only
suite references. New focused state/effect reports pass 21+8 cases. The original
listener requirement is explicitly qualified by current ADR 0006. Test execution,
source/design evidence, native producer authority and manual review remain
separate. The complete delivery audit, candidate memory failure and all real
performance/manual gates stay open; no acceptance checkbox is closed here.

2026-09-11 — User asked whether the budget failure is a leak. A
[same-browser20-cycle diagnostic](../verification/08-memory-growth/README.md)
compares disabled control with repeated enable/LRU/disable, recording RSS and
post-GC heap/DOM counts. DOM/listener counts remain stable after cycle5; retained
heap and RSS still increase modestly, so neither a leak nor a leak-free plateau
is established. The prior32MiB failure remains open; GC interventions are not
acceptance evidence. Production source and normal tests are unchanged.

2026-09-11 — Explicit [browser budget revision](../browser-memory-budget-revision.md):
64 MiB incremental peak RSS per tab, targeting ordinary developer PCs with 1–8
tabs. Retain the 1/8/32-tab release matrix. Historical 32 MiB failures are not
rewritten or relabelled; [new-policy evidence](../verification/08-browser-budget-revision/README.md)
is separate. Repeated-use memory growth, manual reviews and dedicated-host
qualification remain open.
