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

The scanner, immutable revisions, cancellation checks, file tracker and allocation-table traversal, selective page decoders, OOS traversal, exact 16-byte fast-fact encoding, automatic private spill, and bounded envelope workers now exist. Spill records contain only canonical interface-safe facts; their mode-`0600` inode and mode-`0700` session directory are unlinked while open. Worker batches read independently and merge in canonical physical order. Retained diagnostic records are conservatively charged to the resident budget with a terminal-diagnostic reserve; corruption-heavy input stops with an explicit `inspection.resource_policy.diagnostics` boundary rather than growing without limit. Tests prove graph equivalence across resident/spilled storage, one/four worker execution, and corruption-heavy worker counts, explicit partial coverage at spill/diagnostic boundaries, and absence of named artifacts. An exact-commit 192 MiB synthetic snapshot has also yielded the first immutable 31-page acceptance slice under `fixtures/e1e651de/`, covering the current semantic families with hashes and provenance. On that source snapshot, Volmap classifies all 12,288 logical pages and inspects 5,312 reserved/system envelopes; it finds 81 permanent files, 120 heap pages, 43 B-tree pages, four OOS pages, two overflow pages, and the catalog/vacuum/system families without envelope findings. This advances the measurable surface and establishes a reproducible small profile, but does not close the ticket: controlled cache/device runs, larger dense/corrupt/OOS-heavy profiles, cursor queries, and the required distribution of measurements are still missing. Current CLI constants remain development scaffolding rather than accepted production defaults.
