Type: research
Status: claimed
Blocked by: 05

# Measure representative scan performance and set resource-budget defaults

## Question

Using the resolved scan/index/cache architecture and representative small, medium, large, sparse, highly allocated, and corruption-heavy CUBRID snapshots, what numeric version-one defaults and documented expectations should govern resident memory, spill capacity, worker count, fast-scan throughput/latency, page-envelope I/O amplification, query latency, OOS traversal steps, and decoded-byte budgets? Measure the packed index and worst credible diagnostic/evidence growth, verify that automatic spill and cancellation preserve the promised coverage semantics, and recommend formulas or fixed defaults that remain safe on constrained hosts without silently sampling or truncating work.

## Comments

### Standing human disposition and measurement prerequisite

On 2026-08-19 the user directed every remaining ticket to accept the source-backed recommended option and continue without further HITL. The completed [resource-budget research](../research/resource-budgets.md) establishes that no numeric recommendation is evidence-backed yet: the current binary has no discovery, scanner, packed store, spill, query, cancellation, worker, or OOS traversal implementation, and the only local 192 MiB database is mutable and unrepresentative.

The accepted recommendation is therefore to withhold public numeric defaults rather than fabricate them, implement every measurable path with explicit test-supplied `ResourcePolicy` values, create the immutable benchmark corpus, then run the report's matrix and close this ticket from actual data. Exact format/architecture formulas and the benchmark procedure are already recorded in the report. This ticket remains claimed because the question explicitly requires measured numeric defaults; blanket acceptance removes HITL but does not convert missing measurements into evidence.

### Implementation checkpoint

The scanner, immutable in-memory revisions, cancellation checks, file tracker and allocation-table traversal, selective page decoders, and OOS traversal now exist. On the mutable corroboration database the tracker reconciles 98 permanent files and the allocation graph classifies 2,504 allocated, 5,876 reserved-unallocated, 3,904 unreserved, and 4 system pages (12,288 total) without ownership overlap. This advances the measurable surface but does not close the ticket: packed spill, worker execution, cursor queries, the immutable representative corpus, controlled cache/device runs, and the required distribution of measurements are still missing. Current CLI constants remain development scaffolding rather than accepted production defaults.
