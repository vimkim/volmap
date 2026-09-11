# Ticket 08 preparation handoff

**Release readiness: open. No performance case or manual review was executed.**
Work item 140 prepares the available work while ticket 06's named reviews and
08's dedicated-host campaign remain outstanding. Ticket 07 functional completion
is preserved.

## Contents

- [Gate manifest](manifest.json): 29 gate rows, the 50 contract checklist entries
  from tickets 01–05, inherited 06/07 provenance, the unexecuted performance axes
  and a per-run record template. Final named per-invariant mapping/acceptance is
  still open; aggregate suite evidence is explicitly labelled as such.
- [Evidence inventory](evidence-inventory.json): SHA-256 of 126 retained files
  under 06-shared-glyph, 07-develop and 07-develop-disk at preparation time. It
  detects subsequent file changes; it is not original-run attestation.
- [Measurement protocol](../../../../docs/runtime-overlay-release.md): workload
  and host inputs, pairing, thresholds, raw metrics, confidence-method recording,
  manual review and rerun rules.
- [Operator guide](../../../../docs/runtime-overlay.md): startup enablement,
  explicit socket, profile, SSH, refusal/recovery and independent disk/runtime
  interpretation, linking existing detailed lifecycle/resource documentation.

## Prerequisite audit at 08ed947

Read 06's final ticket comment and unfilled manual record before evaluating 07's
completion-audit, manifest and four accepted runtime records. The original 06
pass uses consumer `4f129d82274a71a19370d944939eb50069ada51d`, release builds,
12,288 laid-out cells and a recorded dev2 reference host. Chromium worst p95 is
51.6 ms with State/LRU RSS increments 23.05/25.18 MiB; Firefox is 61 ms with
14.94/10.46 MiB. These are 50 ms sampled summed process-tree RSS measurements,
not instantaneous peaks or the release matrix. Both required named manual
reviews remain missing.

Four of six recorded 06 source fingerprints still match:
`web/e2e/observations.spec.ts` and `src/web/generated/frontend.js` changed.
The other matching source/CSS hashes do not prove the changed bundle/harness
behavior; density and affected lifecycle checks must run on the selected release
candidate. All four 07 source hashes
match. The four accepted real browser reports each execute 12 cases with no
failures/skips, across develop/aligned and Debug/RelWithDebInfo. The 07 audit
also retains independent disk/corpus, controlled native-to-HTTP, credential,
socket/budget, platform-exclusion and external CTP evidence with their limits.

The 07 runtime records accurately name dirty consumer base `15acca5`, executable
hashes and testcase hashes. Implementation commit `e7fa074` and evidence commit
`2a8979b` are separate. Historical functional completion does not magically turn
those runs into clean exact-commit release executions. The release manifest keeps
that qualification open and calls for affected candidate reruns.

No producer repository, prior evidence, parent spec or completed planning map is
edited. The pre-existing change to producer ticket 07 belongs to other work and
is excluded from this commit. Current accepted ADR 0006 permits explicit IPv4
listeners in addition to loopback; the older ticket wording does not undo that
decision. ADR 0008's expanded Volume design is also recorded as a candidate-scope
consideration, not newly implemented or measured here.

## Next executable frontier

1. Select the dedicated host, release configurations, workload driver and database
   reset/precondition method. Record collectors and confidence method before
   gathering data. The current host has not been assumed dedicated.
2. Finish the named [manual review](../06-shared-glyph/manual-review.md), recording
   actual commit/technology and findings; never fill the form from automation.
3. Rebuild the selected clean candidate and rerun affected density/integration
   evidence. Execute every required performance case with the protocol's pairing
   and raw data. Retain failed attempts and complete the per-invariant mapping.
4. Only then decide readiness from the manifest. Any required missing, skipped,
   inconclusive or failing result keeps ticket 08 open.

Preparation verification and independent review are recorded in
[verification.md](verification.md). Local verification is not a measurement or
manual accessibility pass.

## Current-source density follow-up

The [cab4a9b rerun](../08-candidate-density/README.md) now records a Chromium
memory failure (34.54 MiB LRU increment), plus two passing Chromium-only repeats.
The manifest marks candidate density failed; variability is not a repair or an
acceptance pass. Firefox and input timing passed the original rerun. These new
results supplement the older preparation audit without rewriting its evidence.

## Named contract evidence

The [functional ledger](../08-contract-ledger/README.md) replaces aggregate-only
references for all 50 checklist entries from tickets 01–05 with named tests/source
links and explicit clause-specific limits. Two focused frontend reports record
21+8 passing cases. It does not close manual, density, performance or final
delivery gates; inherited source/producer/environment qualifications remain.
