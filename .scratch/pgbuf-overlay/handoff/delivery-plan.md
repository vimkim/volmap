# Cross-repo delivery and gate ownership

Status: planning sequence assembled from the accepted map; implementation and
external publication have not started. This is the delivery plan for
[CBRD-27398](http://jira.cubrid.org/browse/CBRD-27398) and the
[Volmap entry](volmap-entry.md), with the [CUBRID entry](cubrid-entry.md).

## Dependency order

| Stage | Owner and output | Depends on | Exit evidence |
| --- | --- | --- | --- |
| Contract consolidation | CUBRID producer owner: versioned semantic schema/corpus in planned `docs/pgbuf-inspector/v1/`; Volmap owner: exact vendored copy in `fixtures/pgbuf-inspector-v1/` | Completed planning handoff | Independent encoding/decoding cases and corpus revision/hash agreement; concrete syntax implements, rather than reopens, accepted semantics |
| Bounded state producer | CUBRID owner: parameter, daemon/socket, identity, scan and serializer; engine unit cases and external shell case | Contract consolidation | Named nonempty unit runs; controlled real-socket debug/release checks and resource-limit cases |
| Shared consumer boundary | Volmap owner: UDS/simulated adapter, shared broker and runtime HTTP resources | Contract consolidation; real integration waits for producer | Real-parser scripted UDS and HTTP tests, coalescing/admission/age/authority/cap proofs, no disk-state changes |
| Browser projection | Volmap browser owner: state marks, LRU mode and accessible detail | Normalized broker/HTTP contract | Semantic tests in both browsers, density/response/memory gates and manual review |
| Cross-repo integration and port | Integration owner with CUBRID owner: format-aligned integration and develop producer port | Producer and consumer, then browser for complete UI gate | Exact build/data/corpus/commit matrix; develop page-kind parity and debug/release lifecycle evidence |
| Release and publication readiness | Integration owner collects gates; issue/PR owner assembles reviewed publication material | All feature paths and required tests | Dedicated-host performance, complete gate manifest, upstream/socket approval and reviewed docs |

Independent simulator/contract work can proceed without waiting for a live
engine, but it does not satisfy real-engine checks. Full visual delivery
follows the normalized state path. Protected resident-page inspection, AOUT,
flush events and TUI runtime parity are not dependencies of any stage here.

## Planned verification artifacts

Paths below are implementation assignments and may be moved together with
updated manifest references. No listed test or evidence file is claimed to
exist today. The role owner is responsible for implementation; named human
assignment can occur when implementation tickets are created.

| Gate | Owning repository / role | Planned test or evidence location |
| --- | --- | --- |
| Producer state/serializer/caps | CUBRID test owner | `unit_tests/pgbuf_inspector/` |
| Shared framing/semantics | CUBRID producer and Volmap adapter owners | `docs/pgbuf-inspector/v1/` and `fixtures/pgbuf-inspector-v1/` |
| UDS, scheduling, cancellation and cache | Volmap runtime owner | `tests/runtime_observation.rs` plus private module unit tests |
| HTTP security/admission and unaffected disk inspection | Volmap HTTP owner | `tests/runtime_http.rs` and existing `src/web.rs` test harness |
| Browser scope/effect behavior | Volmap browser owner | `web/src/model.test.ts`, `web/src/runtime.test.ts` |
| Rendering, accessibility and density | Volmap browser owner and manual reviewer | `web/e2e/page-buffer-overlay.spec.ts`; review notes under the run artifact directory |
| Actual producer socket and controlled engine fixtures | CUBRID external testcase owner | Shell-suite-relative `pgbuf_inspector/CBRD-27398/`, pinned to its actual repository revision |
| Cross-repo real-engine and performance driver | Integration owner | Planned Volmap `release/check-pgbuf-overlay.sh`, using explicit producer/build/database inputs |
| Complete delivery evidence | Integration owner | Planned `release/evidence/pgbuf-overlay/<run-id>/manifest.json` with raw logs/samples and manual-review notes |

The driver must not assume a local personal worktree path or conflate
format-aligned integration with develop disk-format support. Resolve the
actual testcase checkout and build inputs explicitly. Where suitable, reuse
existing local/release test runners, but do not report them as hosted CI.

## Evidence contract

The manifest records requirement identifier, owning test/path, producer and
consumer commits, corpus revision/hash, build mode, testcase revision,
environment, fixture preconditions, executed command/count, result and raw
artifact paths. Manual checks record reviewer and browser/assistive technology.
Quantitative checks also record workload, baseline, raw samples and confidence
calculation. Sensitive process/path/application data must not escape through
user-facing HTTP or UI output; keep reviewed diagnostic artifacts appropriately
scoped and sanitized for publication.

Use the complete [verification](../issues/11-define-verification-strategy.md)
and [budget](../issues/15-set-overlay-resource-budgets.md) contracts. In
particular, the release matrix includes:

- 512 MiB, 1 GiB and 4 GiB pools at 16 KiB/page; idle, read-heavy,
  write/flush-heavy and churn; 1/8/32 tabs; stall, viewport rotation,
  pause/hidden, clock changes and restart.
- Ten paired runs per performance case, 60 s warmup and at least 5 min
  measurement, extending to 100,000 transactions where applicable. A 95%
  confidence upper bound must meet the 2% throughput-loss and 5% transaction
  p99-increase gates. Inconclusive results are not passing.
- The 1 GiB idle/read-heavy reference requires at least 99% complete scans,
  p95 refresh at most 250 ms and cached HTTP at most 25 ms; concurrent disk
  inspection p95 regression at most 5%. Larger pools require honest partial
  coverage, not fabricated completeness.
- Default-cadence producer/broker CPU at most 20%/50% of one logical core;
  incremental peak RSS at most 16/192 MiB respectively and 32 MiB per browser
  tab. Allocation ceilings are independent and still enforced.
- Actual rendered 10,000-page density in Chromium and Firefox, p95
  input-to-visible update at most 100 ms, semantic/non-color/keyboard checks,
  reduced motion/high contrast and manual screen-reader/visual evidence.

Disabled and enabled-without-demand overhead are measured separately. Test
below/at/above all resource limits with retained/in-flight/response allocations
present together. Missing, skipped, failing or inconclusive required evidence
keeps delivery blocked. Code/corpus changes require affected checks to rerun.

## Planning closure versus downstream gates

This map can close when the local entries, keyed draft and delivery plan have
been reviewed against all accepted decisions. That closes design assembly,
not feature delivery. It does not claim compiled code, passed tests, upstream
review or socket approval. The downstream owner must meet those gates and
use the actual implementation HEAD for publication material.

Start Volmap's next flow with `/to-spec` against this focused handoff, then
`/to-tickets` and `/implement`. Start the CUBRID issue/PR flow using the keyed
draft and producer plan; publication is a separate authorized workflow, and
creating the draft PR still requires an implemented source branch. Do not
publish a planning-only branch as if it implements this feature.
