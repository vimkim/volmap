# CUBRID producer and develop-PR handoff

Status: planning artifact; no engine implementation, test result or published
PR is implied. The target issue is
[CBRD-27398 — PGBUF overlay](http://jira.cubrid.org/browse/CBRD-27398), fetched
as an Open Sub-task under CBRD-27193 and assigned to Daehyun Kim. Preserve
those live issue relationships; this handoff does not change JIRA metadata.

The local Korean issue draft is
`CBRD-27398-pgbuf-overlay_cd593bc_codex.md` in the configured
`my-cubrid-jira/issues/` repository. Its source suffix refers to inspected
CUBRID commit `cd593bcf2d8643b4698f1cb311c4c23af23a9d57`, not a future
implementation HEAD. The [handoff index](README.md) links the accepted
decision authority; the draft is a portable consolidation for the issue.

## Producer ownership

| Implementation owner | Planned landing points | Contract |
| --- | --- | --- |
| CUBRID producer implementer | `src/storage/page_buffer.c`; planned `src/storage/pgbuf_inspector.cpp` and `.hpp` | Semantic bounded BCB sampling, scan/serializer, private daemon and socket lifetime; no page-content capture or hot-path instrumentation |
| CUBRID lifecycle/parameter implementer | `src/base/system_parameter.c/.h` and existing server startup/shutdown wiring | Startup-only hidden Boolean, default off; supported Unix SERVER_MODE release/debug builds; activation failure leaves DB startup intact |
| CUBRID test implementer | Planned `unit_tests/pgbuf_inspector/` and `docs/pgbuf-inspector/v1/` | Actual executable test registration/invocation and producer-owned versioned corpus |
| External testcase owner | Planned case subtree `pgbuf_inspector/CBRD-27398/` under the external shell-suite root | Parameter/socket/security/restart tests and controlled engine fixtures; actual repository revision recorded in the evidence manifest |
| Cross-repo integration owner | Dedicated integration driver and evidence directory specified in [delivery plan](delivery-plan.md) | Format-aligned Volmap integration, develop producer checks, release performance and exact-commit evidence |

Landing points are planned, not existing implementations. File naming is
private organization; accepted source boundaries, safety and behavior are
not optional. The external suite root must be resolved from the checked-out
testcase repository, not fabricated as a local engine `tests/` directory.

## Engine implementation constraints

Follow [gating](../issues/04-define-gating-matrix.md),
[wire v1](../issues/05-define-wire-contract-v1.md),
[LRU semantics](../issues/14-expose-exact-lru-list-membership.md),
[security](../issues/08-set-security-posture.md) and
[budgets](../issues/15-set-overlay-resource-budgets.md) in full.

The producer emits semantic scalar observations, not raw BCB structures or
page-type ordinals. Latch mode/waiter/fix count share one atomic latch-word
read; LRU zone/kind/index share one flags read. Neither tuple makes the
whole page or pool atomic. Complete-scan omission and partial unknown remain
different. A valid footer is required even when a scan stops at a budget.

No new runtime JSON dependency is introduced. No inspector-specific compile
option is added for v1, and no endpoint is exposed by Windows or non-server
binaries. Future capture/transition instrumentation remains separately gated
and outside this delivery. Preserve CUBRID error handling, source indentation,
include ordering and existing server lifecycle conventions.

This is not a holder-tracking or timeout-diagnosis implementation of the
CBRD-26325 instrumentation proposal. That prior art motivates diagnostic
visibility but does not authorize thread identities, pointers, stack capture,
per-fix accounting or application bytes in this protocol.

## Branch and PR plan

1. Revalidate the chosen base and working-tree state before implementation.
   The accepted alignment starts iteration/integration from the Volmap-pinned
   `e1e651d` format baseline, then ports the producer to develop. Do not use
   the analysis worktree's current HEAD as an implicit implementation branch.
2. Implement producer-owned conformance fixtures and the bounded producer
   together. Volmap vendors the corpus by exact revision and content hash;
   no test-time network dependency is introduced.
3. Complete debug/release integration against format-matched data, then
   cherry-pick the producer change onto a develop-based issue branch.
   Recheck semantic page-kind mappings and run develop producer tests;
   previous branch-parity evidence is not a substitute for the delivered diff.
4. Prepare the later draft PR against `CUBRID/CUBRID:develop`, with head on
   the validated `vimkim/cubrid` fork. Planned title:
   `[CBRD-27398] Add bounded page-buffer state inspector`.
   The concrete head branch and SHA are bound only after implementation.
5. At publication time, create the detailed explanation using the actual
   implementation HEAD, not `cd593bc`. Use the CUBRID PR flow's English
   `Purpose`, `Implementation`, `Remarks` sections with Korean prose and
   explicit AS-IS/TO-BE. Link the real JIRA issue and pushed detailed document;
   do not publish placeholder URLs or machine-local handoff paths.
6. Run the publication flow's material checker and review before its push/PR
   steps. Keep the PR draft and distinguish unexecuted gates from passing
   evidence. This plan performs none of those external writes.

The eventual implementation must obtain upstream acceptance of the new socket
and satisfy the release gates. If review rejects an accepted design boundary,
revisit that decision explicitly rather than silently substituting SHOW,
shared memory or a remote transport.

## Review and verification handoff

The [verification decision](../issues/11-define-verification-strategy.md)
assigns the required layers and oracles. In particular, a successful compile
or empty ctest run is not evidence that Catch2 cases ran. The external shell
case must be delivered with the engine change and identified by repository
revision. Lack of private-suite access is missing evidence, not a pass.

Controlled known-VPID fixture conditions must be established independently
of the inspector and held across the relevant scan. Real-socket checks use
the shipped producer path; fixture machinery does not replace unmodified
release attachment/lifecycle checks. Failure to establish the precondition
is inconclusive. Observing cannot fix a page or mutate the inspected state.

Published test instructions must use the project's actual build/test tools
or clearly describe verification concepts. Personal task-runner recipes,
aliases and absolute workspace paths are not CUBRID reviewer instructions.
All measurements and manual accessibility evidence remain future work.
