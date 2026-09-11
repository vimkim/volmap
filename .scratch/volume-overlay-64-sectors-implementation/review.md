# Implementation planning review

Status: published — user approved test seams and ticket breakdown on2026-09-11.

## Published specification and tickets

- [Implementation specification](spec.md)
- [01: Sector scope with retained detail](issues/01-observe-sector-scope.md)
- [02: Volume64-sector slim overlay](issues/02-display-volume-64-sectors.md)
- [03: Lifecycle and multi-tab admission](issues/03-preserve-lifecycle-under-multitab-demand.md)
- [04: Browser density and accessibility](issues/04-qualify-browser-density-and-accessibility.md)
- [05: Runtime resource budgets](issues/05-qualify-runtime-resource-budgets.md)

## Approved verification seams

Primary: real HTTP observation flow with the existing authenticated fixture producer,
verified through the actual browser client. Supplement: existing virtual-clock broker
lifecycle seam for exact deadlines, cancellation and race ordering. No new test-only
service boundary or wide prefactoring is proposed.

Source evidence checked on2026-09-11:

- HTTP boundary and shared scan: `src/web.rs`, tests for bounded request bodies and
  concurrent HTTP scopes sharing a scan while retaining independent coverage/admission.
- Time/cancellation: `src/web/observations/lifecycle_tests.rs`, caller cadence,
  post-resume start barrier, paused metadata/expiry, restart, cancellation and partial scans.
- Presentation/geometry: `web/src/observation-view.tsx` and `web/src/view.tsx`;
  scope visibility, fixed-slot previews, projection-specific labels and memoization.
- Existing browser integration: `web/e2e/real-producer.spec.ts`,
  `web/e2e/overlay-accessibility.spec.ts`, `web/e2e/overlay-density.spec.ts`.

These are navigation pointers for implementers to verify against their current baseline,
not frozen source-line requirements. The implementation specification and ticket criteria express
the enduring contracts without coupling them to these implementation paths.

## Dependency reasoning

| Ticket | Blocked by | Independently demonstrable outcome |
| --- | --- | --- |
|01|None|Sector-level request produces the same detailed64-page UI through the new view-scope contract.|
|02|01|Volume uses that addressing/envelope contract for64 visible sectors, slim results and full HTTP/UI validation.|
|03|02|Mixed Volume/Sector/Page tabs preserve lifecycle, overload and memory admission under adversarial demand.|
|04|03; browser/measurement prerequisites|The correct4,096-page flow meets browser performance and accessibility gates.|
|05|03; real-producer/measurement prerequisites|The same flow meets server CPU/RSS/latency and workload-overhead gates.|

04 and05 do not block one another. Changes to shared code after a measurement require
affected results to be revalidated on the final revision. Both are required for overall
completion.01/02 include basic lifecycle and bound checks;03 does not license deferral of
fundamental correctness.04/05 can prepare harnesses while external execution prerequisites
are outstanding, but cannot close unexecuted gates.

## Source decisions to load

For accepted product policy and investigation, read
[design specification](../volume-overlay-64-sectors/spec.md),
[completed decision map](../volume-overlay-64-sectors/map.md), and
[ADR0008](../../docs/adr/0008-scope-runtime-projections-to-view-level.md).
For terminology, read [CONTEXT](../../CONTEXT.md).
For lifecycle and safety boundaries, read
[ADR0006](../../docs/adr/0006-runtime-observations-are-loopback-web-capabilities.md) and
[ADR0004](../../docs/adr/0004-separate-runtime-observation-from-resident-inspection.md).
For inherited performance thresholds and measurement methodology, read
[resource budget decision](../pgbuf-overlay/issues/15-set-overlay-resource-budgets.md).
ADR0008 changes the old512-page Volume observation cap; it preserves byte/memory/cadence budgets.

## Publication record

2026-09-11 — The user approved the existing test seams and five-ticket breakdown.
The specification and five individual tickets are published locally with
`ready-for-agent` status. The historical design specification and completed decision
map remain unchanged. This phase includes no implementation, commit or external posting.

The execution frontier is ticket01. Complete01, then02, then03;04 and05 may then
proceed independently when their stated external prerequisites are available.
Each fresh implementation session should load its ticket, the implementation spec,
and the context pointers above. Ready-for-agent does not mean its blockers are satisfied.
