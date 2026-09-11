# State-only page-buffer overlay handoff

Status: planning handoff complete and reviewed. Implementation and external
publication have not started.

This index collects the accepted decisions from
[Chart the live page-buffer overlay across volmap and CUBRID](../map.md).
It does not authorize implementation or external publication. Both repository
entry points and their delivery plan are assembled; the map's design work is
complete, while the release gates below remain unexecuted.

## Artifact readiness

The focused [ready-for-agent implementation specification](../../pgbuf-overlay-implementation/spec.md)
has now been synthesized in the local tracker. Use it for `/to-tickets`;
the entries below retain handoff rationale and cross-repository context.

The companion [ready-for-agent CUBRID producer specification](../../pgbuf-producer-implementation/spec.md)
consolidates the engine, protocol corpus and producer verification requirements.
Use that specification for the separate producer `/to-tickets` graph.

| Artifact | Current state | Completion requirement |
| --- | --- | --- |
| Volmap implementation entry | [Focused state-only entry](volmap-entry.md) reviewed | Ready for `/to-spec`; resident-page inspection is not a prerequisite |
| CUBRID JIRA draft | [CBRD-27398 local draft](/home/vimkim/gh/my-cubrid-jira/issues/CBRD-27398-pgbuf-overlay_cd593bc_codex.md) revised and approved | Ready for the later issue workflow; no upload performed |
| Develop PR plan | [CUBRID entry and PR plan](cubrid-entry.md) reviewed | Implementation branch and publication evidence are downstream, not an open planning question |
| Cross-repo delivery plan | [Delivery order and evidence ownership](delivery-plan.md) reviewed | Ready to create implementation tickets; runtime gates remain unexecuted |

Draft review corrected evaluated-page completeness, scope/epoch adoption,
explicit develop debug/release coverage and terminology. The reviewer then
approved the corrected draft and cross-repo consistency. Local link,
whitespace and JIRA portability/heading checks passed. These checks validate
the documents, not runtime behavior.

An older standalone `volmap-cubrid-bcb-inspector-handoff.md` in the CUBRID
analysis worktree still describes gating/security/verification as open. It
was left untouched as pre-existing material; this reviewed map and handoff
supersede that historical status for the current effort.

## Decision authority

Details remain authoritative in their accepted decision tickets. A later
implementation may choose private helper names but cannot silently weaken
these contracts.

| Concern | Authority |
| --- | --- |
| Source evidence and branch alignment | [Exposure parity](../issues/01-verify-branch-exposure-surface.md), [target branch and format alignment](../issues/02-choose-target-branch.md) |
| Transport, producer gating and semantic records | [Transport](../issues/03-choose-transport-channel.md), [gating matrix](../issues/04-define-gating-matrix.md), [wire v1](../issues/05-define-wire-contract-v1.md), [exact LRU membership](../issues/14-expose-exact-lru-list-membership.md) |
| Socket, identity and HTTP attachment security | [Security posture](../issues/08-set-security-posture.md) |
| Domain terms, display and broker lifecycle | [Vocabulary and ADR](../issues/06-name-domain-terms-and-adr.md), [visual encoding](../issues/09-prototype-heatmap-encoding.md), [overlay architecture](../issues/10-define-volmap-overlay-architecture.md) |
| Limits and required evidence | [Resource budgets](../issues/15-set-overlay-resource-budgets.md), [verification strategy](../issues/11-define-verification-strategy.md) |
| Excluded capabilities | [TUI parity](../issues/07-decide-tui-parity-treatment.md), [consistency inspection](../issues/13-choose-consistency-inspection-boundary.md), [AOUT history](../issues/16-decide-aout-overlay-scope.md), [flush-transition events](../issues/17-decide-transition-changefeed-scope.md) |

## Delivery boundaries

The producer lands on develop, with format-aligned iteration/integration as
specified by the branch decision. Volmap consumes semantic state through its
private adapter; browser code does not learn the producer protocol or CUBRID
private layouts. The producer-owned conformance corpus is vendored by exact
revision/hash into Volmap, with independent offline tests on both sides.

State-only scans do not load missing pages, copy page contents, establish
transaction visibility or prove durability. Sampled flushing fields remain;
event hooks, replay, causal timelines and AOUT collection do not. Protected
page capture, disk/DWB comparison, digests and TDE normalization belong to a
later effort. Runtime observations remain web-only and independent of disk
inspection facts, outcomes and diagnostics.

## Planning completion is not release approval

None of the following is claimed as completed by resolving this map:

- Implemented producer, UDS adapter, broker, HTTP or browser behavior.
- Executed conformance, lifecycle, security, debug/release or browser tests.
- Passed CPU, memory, latency, throughput or completeness measurements.
- Completed manual accessibility review or exact-commit gate manifest.
- CUBRID organization approval of the new socket, upstream review acceptance,
  or access to the external testcase infrastructure.
- JIRA publication, source/document commits or pushes, or an opened PR.

These remain explicit downstream gates under the accepted verification
strategy. Missing, skipped and inconclusive required evidence is not passing.
The final assembly must give each gate an owner and an artifact path without
inventing results or treating local scripts as hosted CI.
