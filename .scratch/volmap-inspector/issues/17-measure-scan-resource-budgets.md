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
