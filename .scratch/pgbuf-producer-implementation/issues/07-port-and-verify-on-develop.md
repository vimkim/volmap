# 07: Port and verify on develop

**What to build:** A develop-based CUBRID server provides the same semantic observation contract, with producer functionality independently verified in debug and release after the accepted format-aligned integration.

**Blocked by:** 06 — Integrate with the Volmap consumer.

**Status:** ready-for-agent

- [ ] Revalidate the current develop base and worktree state, create an isolated issue branch, port the completed producer/corpus/tests and preserve unrelated changes. Record exact source and target commits and any intentional conflict resolutions.
- [ ] Revalidate BCB scalar access/lifetime, coherent latch/LRU decoding, daemon initialization/destruction and identity acquisition on develop. Preserve formatting, include ordering, CUBRID error conventions and the absence of observation hot-path changes.
- [ ] Check every branch-specific page-kind mapping against the same semantic corpus. Develop does not emit oos or leak shifted native ordinals. A semantic match cannot establish compatibility of develop volume bytes with Volmap.
- [ ] Execute producer conformance/unit tests and actual socket/controlled-state tests on develop in both debug and release. Re-run relevant admission, security, cancellation, activation-failure, restart and resource-bound cases on the port; source-branch results alone do not prove the delivered diff.
- [ ] Verify the accepted build/gating matrix: supported Unix SERVER_MODE release/debug variants with default-off startup-only parameter, no new v1 feature build option, and no endpoint in Windows/non-server binaries. Record unsupported validation environments explicitly rather than claiming them passed.
- [ ] Bind external companion testcase revision, corpus hash, executed commands/counts and raw evidence to the develop implementation. Preserve the format-aligned build evidence separately; changes during port invalidate affected evidence and require reruns.
- [ ] Leave a reviewable develop implementation and evidence for ticket 08. Do not create a JIRA update, source push or PR solely because this ticket exists; external publication uses its later authorized workflow.

## Source and scope

Implements the approved CUBRID producer specification for CBRD-27398 in this feature tracker. Read its full contract and linked decision authority before implementation. The public protocol is the main test boundary; focused deterministic scan/serializer checks supplement real I/O. This ticket does not add resident-page capture, AOUT or event history, native LRU telemetry, or Volmap UI work.

## Comments

2026-09-09: Published after the user approved the eight-ticket breakdown and blocking edges. Ready-for-agent describes triage readiness; blockers and acceptance criteria remain unsatisfied until evidenced.
