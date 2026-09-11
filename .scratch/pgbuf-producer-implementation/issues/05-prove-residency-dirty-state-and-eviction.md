# 05: Prove residency, dirty state and eviction

**What to build:** A reviewer can reproduce known resident, dirty and evicted page observations through the shipped socket, using independently controlled engine conditions rather than timing guesses.

**Blocked by:** 03 — Observe a bounded resident-set scan; external prerequisite: access to the actual external CUBRID shell testcase repository and an environment able to run its companion case.

**Status:** complete

- [x] Create controlled test-only fixtures with known VPIDs and bounded synchronization. Establish residency and dirty-state preconditions independently of inspector responses, hold them across observation and record the synchronization evidence.
- [x] For eviction, independently prove removal and prevent refixing until a complete scan finishes. Assert complete-scan omission supports observed nonresidency while partial omission remains unknown. Failed preconditions are inconclusive; sleeps and the inspector response itself are not precondition oracles.
- [x] Exercise observable semantic state and framing through the shipped producer path. Fixtures may control workload state but the observer must never fix, mutate or load pages. Add no production control endpoint, event hook or permanent per-fix accounting.
- [x] Deliver a companion shell testcase in the actual external testcase repository, with exact revision and reproducible standard test invocation. Resolve its real checkout rather than inventing an engine-local shell-suite location.
- [x] Include parameter-off absence, enabled real-socket observation, complete/partial scans and restart in the companion flow. Reuse the credential/lifecycle harness where appropriate without making this ticket depend on ticket 04; that independent adversarial evidence is joined by ticket 06.
- [x] Execute controlled cases on the format-aligned producer in debug and release, and separately check unmodified release attachment/lifecycle so test fixture machinery cannot replace production verification.
- [x] Record fixture preconditions, producer and testcase revisions, build mode, commands, named case counts and raw artifacts. Missing private-suite access or failed fixture synchronization keeps this ticket incomplete. Supply reusable fixtures for develop verification in ticket 07.

## Source and scope

Implements the approved CUBRID producer specification for CBRD-27398 in this feature tracker. Read its full contract and linked decision authority before implementation. The public protocol is the main test boundary; focused deterministic scan/serializer checks supplement real I/O. This ticket does not add resident-page capture, AOUT or event history, native LRU telemetry, or Volmap UI work.

## Comments

2026-09-09: Published after the user approved the eight-ticket breakdown and blocking edges. Ready-for-agent describes triage readiness; blockers and acceptance criteria remain unsatisfied until evidenced.

2026-09-09 implementation checkpoint: Native clean-held, dirty-held, partial-held and shutdown cases run through the production daemon/socket. Debug and Release focused CTests pass; actual isolated external CTP executes one successful companion case per mode, including an independent unmodified server attachment/restart run. Eviction scope (explicit `pgbuf_invalidate` removal versus actual LRU replacement) remains a pending user clarification, so omission evidence, final full suites, two-axis review and commits are not complete. See `../verification/05/progress.json` and `../verification/05/checkpoint.md`.

2026-09-09 continuation: Implemented actual capacity-driven buffer replacement, so no decision to narrow eviction to explicit invalidation is needed. Native target miss is proven before cleanup; cleanup explicitly excludes the still-allocated target. Debug native semantic cases and all 28 CTest entries pass. Standards/Spec review has zero findings; engine and testcase commits are recorded while final Release/CTP gate verification runs.

2026-09-09 completed: engine `f8c068f771ccd141a3c2f08541fe75c84e41ce53`, external testcase `a648d78f599504fe0c916628ea51ea3f2b5f7ca7`. Final Debug and RelWithDebInfo each pass all 28 CTest entries (61 inspector cases plus six native checks); actual external CTP executes one successful case per mode with zero failures/skips. Separate unmodified Release attachment/lifecycle passes. Standards and Spec reviews have zero findings. Commits, reviewed trees, binary hashes, bounded native synchronization, raw complete/partial captures, failed-attempt history and standard invocation are recorded in `../verification/05/manifest.json`; `../verification/05/checkpoint.md` maps every acceptance item to evidence.
