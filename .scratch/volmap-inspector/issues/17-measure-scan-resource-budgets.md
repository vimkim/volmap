Type: research
Status: resolved
Blocked by: 05

# Measure representative scan performance and set resource-budget defaults

## Question

Using the resolved scan/index/cache architecture and representative small, medium, large, sparse, highly allocated, and corruption-heavy CUBRID snapshots, what numeric version-one defaults and documented expectations should govern resident memory, spill capacity, worker count, fast-scan throughput/latency, page-envelope I/O amplification, query latency, OOS traversal steps, and decoded-byte budgets? Measure the packed index and worst credible diagnostic/evidence growth, verify that automatic spill and cancellation preserve the promised coverage semantics, and recommend formulas or fixed defaults that remain safe on constrained hosts without silently sampling or truncating work.

## Comments

### Standing human disposition

On 2026-08-19 the user directed every remaining ticket to accept the source-backed recommended option and continue without further HITL. Missing measurements were still treated as missing evidence, so this ticket remained claimed until the executable matrix existed.

## Answer

Resolved by [Resource-budget measurements and version-one defaults](../research/resource-budgets.md) and benchmark commit `2c19cca`.

Accept 256 MiB admitted resident memory, 2 GiB private spill, four workers, 16,384 chain steps, and 256 MiB decoded input as the internal version-one defaults. The scan reads exactly the 32-byte prefix and 8-byte watermark, stores exact 16-byte facts, spills automatically, merges workers canonically, and publishes explicit partial coverage on every tested resource or cancellation boundary.

Thirty-sample warm-cache measurements cover small, medium, 1 GiB large, 4 GiB sparse, 512 MiB fully reserved, corruption-heavy, resident/spilled, one/four-worker, query, cancellation, and 512-chunk complete/cyclic OOS profiles. Large/dense spill results preserved the same graph as resident storage. Four workers materially improved the representative large, dense, and corruption-heavy medians. Exact step and byte limits published 511 chunks while the boundary value completed all 512.

Do not turn the reference-host timings into a universal SLO. Controlled cold-cache, constrained-host, cross-distribution, larger semantic/fuzz, and company approval checks remain public-release gates.
