Label: wayfinder:map
Status: open

# Chart the live page-buffer overlay across volmap and CUBRID

## Destination

An implementation-ready cross-repo specification for live page-buffer state visualization: a versioned inspector wire contract; a CUBRID-side design ready for the JIRA + develop-PR flow (system-parameter-gated, debug-safe); and a volmap-side overlay, vocabulary, rendering, and parity spec ready for `/to-spec`. The map is complete when no branch, transport, gating, security, vocabulary, parity, encoding, or delivery-order decision remains implicit before implementation begins.

## Notes

- Planning only: this map produces decisions, not deliverables. No auto-answer directive is in force — grilling tickets resolve only with the user present.
- Permission grant (user, 2026-08-21): CUBRID code that interacts with volmap may be modified when needed, in debug mode only where it degrades performance, and gated by a system parameter such as page-buffer monitoring.
- Standing design reference: [docs/live-page-buffer-inspection.md](../../docs/live-page-buffer-inspection.md) — semantic inspector over a Unix-domain socket, explicit classifications, optimistic capture bracket, delivery order. Tickets refine it into decisions; they do not re-derive it.
- Charting surveys (2026-08-21): [Volmap seams for a live page-buffer overlay](research/volmap-live-overlay-seams.md) and [CUBRID page-buffer exposure surface](research/cubrid-pgbuf-exposure-surface.md). The CUBRID survey ran on an OOS worktree; per-branch re-verification is [ticket 01](issues/01-verify-branch-exposure-surface.md).
- External evidence: `/home/vimkim/gh/my-cubrid-docs/pgbuf-analysis/e6ed61e_claude/06-misc-observability.md` (pgbuf observability layers) and `/home/vimkim/gh/my-cubrid-docs/cbrd-26325/CBRD-26325-instrumentation-proposal.md` (in-house prior art for per-page instrumentation).
- Volmap pins the feat/oos volume format at `e1e651d`; the requested inspector landing branch is develop — the tension is ticket 02.
- Skills: HITL sessions consult `grilling` + `domain-modeling`; module-boundary decisions consult `codebase-design`; UI decisions use `prototype` (and `dataviz`); AFK facts use `research`. The CUBRID handoff flows through `cubrid-jira-issue-write` and `cubrid-pr-create`; the volmap handoff flows through `/to-spec` → `/to-tickets`.
- Tracker: local markdown in this directory. Claim a ticket by setting `Status: claimed (<who>)` before working it.
- Durable work-tracker item: `27`. Overlap alert: item `24` ("Design Volmap interactive live web architecture") lists cub_server page-buffer observation in its scope; the user decides whether that item defers the buffer-observation dimension to this map.

## Decisions so far

- [Verify the page-buffer exposure surface on the candidate CUBRID branches](issues/01-verify-branch-exposure-surface.md) — all seven surveyed facts hold on both branches (develop already has the atomic latch, so a coherent latch/fcnt read needs no BCB mutex anywhere); one wire contract serves both, provided page kind is a semantic vocabulary since OOS's mid-enum `PAGE_OOS` shifts raw ptype values ≥ 8.
- [Choose the CUBRID target branch and format alignment](issues/02-choose-target-branch.md) — land the inspector on develop; iterate and demo on a working branch forked from the volmap-pinned `e1e651d` so demodb matches volmap's format authority; cherry-pick the finished producer to develop (trivial known conflicts); enum-shift risk re-verified in code and absorbed by the semantic page-kind vocabulary. Item 24 defers buffer observation to this map.
- [Choose the inspector transport channel](issues/03-choose-transport-channel.md) — AF_UNIX socket, `SOCK_STREAM`, versioned JSON-lines (handshake, then newline-delimited records); zero new dependencies on either side; the per-BCB SHOW variant is deferred out of v1.
- [Define wire contract v1](issues/05-define-wire-contract-v1.md) — state-only bulk resident-set scan (`NOT_RESIDENT` by omission), twelve semantic fields (LRU index excluded), incarnation-bound handshake with `scan_seq` bracketing, additive snake_case JSON conventions, 2-client / 100 ms-floor / disconnect-on-stall limits with stable refusal codes; point `InspectPage` deferred to the digest phase as an explicit deviation from the design reference's point-first order.

## Not yet specified

- Consistency classification and digest evidence — the design reference's `SYNCHRONIZED_WITH_*` classifications, TDE normalization, main-volume/DWB digests, and the optimistic capture bracket. Hangs on wire contract v1 and the gating matrix; explicitly not faked in the state-only phase.
- DWB and AOUT observation layers — the per-VPID DWB probe has a different locking cost profile than the lock-free BCB scan; "recently evicted" (AOUT) as a possible heatmap layer.
- Transition changefeed for flush-path causality (design reference extension 3).
- Independent volmap disk cross-check (design reference extension 2).
- Poll cadence, scan cost, and performance budgets — measurement-backed defaults, in the spirit of the volmap-inspector map's resource-budget ticket. Hangs on transport and overlay architecture.

## Out of scope

- Implementing the feature inside this map; the map hands off to `/to-spec` (volmap) and the JIRA/PR flow (CUBRID).
- Making volmap a transaction-visibility or committed-state tool; the overlay explains engine residency, never commit state.
- Any engine write or repair path; the inspector never mutates database state.
- TCP or remote exposure of the inspector channel; remote use goes through SSH forwarding (design reference).
- Always-on production monitoring; the default-off parameter and debug gates stand.
- Shared-memory raw-struct export and raw process-memory traversal (rejected by the design reference).
- A per-BCB `SHOW` statement variant — deferred out of v1 by [ticket 03](issues/03-choose-transport-channel.md); the JIRA issue may cite it as future work, and it remains the fallback position if upstream review resists the socket.
