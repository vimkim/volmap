Label: wayfinder:map
Status: resolved

# Chart the live page-buffer overlay across volmap and CUBRID

## Destination

An implementation-ready cross-repo specification for a state-only live page-buffer overlay: a versioned state-observation wire contract; a CUBRID-side design ready for the JIRA + develop-PR flow (system-parameter-gated, debug-safe); and a volmap-side overlay, vocabulary, rendering, and parity spec ready for `/to-spec`. The map is complete when no branch, transport, gating, security, vocabulary, parity, encoding, or delivery-order decision remains implicit before implementation begins.

## Notes

- Planning only: this map produces decisions, not deliverables. No auto-answer directive is in force — grilling tickets resolve only with the user present.
- Permission grant (user, 2026-08-21): CUBRID code that interacts with volmap may be modified when needed, in debug mode only where it degrades performance, and gated by a system parameter such as page-buffer monitoring.
- Standing design reference: [docs/live-page-buffer-inspection.md](../../docs/live-page-buffer-inspection.md) — this map refines its state-only semantic-inspector subset; resident-page capture, persistence comparison, explicit classifications, and optimistic capture bracketing are reserved for the later consistency-inspection map.
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
- [Define the gating matrix](issues/04-define-gating-matrix.md) — startup-only hidden `enable_pgbuf_inspector=false`; no daemon/socket when off; the state-only Unix server producer is compiled and release-capable behind the parameter, while future image/digest/I/O/instrumentation capabilities are separately gated and debug-only.
- [Define wire contract v1](issues/05-define-wire-contract-v1.md) — state-only bulk resident-set scan (`NOT_RESIDENT` by omission), semantic per-BCB state, incarnation-bound handshake with LRU topology and `scan_seq` bracketing, additive snake_case JSON conventions, 2-client / 100 ms-floor / disconnect-on-stall limits with stable refusal codes; point `InspectPage` deferred to the digest phase as an explicit deviation from the design reference's point-first order.
- [Expose exact LRU-list membership in wire v1](issues/14-expose-exact-lru-list-membership.md) — every BCB reports `shared|private|none|invalid` plus a kind-local index alongside its zone, all decoded coherently from one flags sample; topology counts are incarnation-scoped, observed summaries are derived from scan records, and native list telemetry remains excluded.
- [Choose the consistency-inspection boundary](issues/13-choose-consistency-inspection-boundary.md) — this map stops at the state-only runtime overlay; protected page capture, persistence evidence, digests, TDE normalization, and consistency classification move together to a later Wayfinder map without weakening wire v1.
- [Set the security posture of an engine-connected serve](issues/08-set-security-posture.md) — runtime attachment is double-opt-in and loopback-only; both peers require an exact effective-UID match, bind a private safely reclaimed socket, prove the complete persistent volume identity plus server incarnation, fail inspector activation closed while allowing database startup, and disclose no paths or process identities.
- [Name the volmap domain terms and the volatile-overlay ADR](issues/06-name-domain-terms-and-adr.md) — retain the existing runtime-observation vocabulary and ADR-0006: page-buffer readings are independently timed, volatile, web-only capabilities outside inspection facts, revisions, exports, and terminal parity; optional sources remain additive, and absence, refusal, or incompatibility leaves ordinary disk inspection unchanged with no overlay.
- [Prototype the heatmap visual encoding](issues/09-prototype-heatmap-encoding.md) — preserve storage colors under cyan residency outlines, amber dirty corners, and static magenta flushing edges; provide a separate LRU-topology color mode with exact list indices in Page detail; label fresh/stale/paused/absent capability states explicitly, and keep bursty latch facts out of the dense grid.

- [Define the volmap overlay architecture](issues/10-define-volmap-overlay-architecture.md) — one demand-driven broker shares bounded bulk scans across browser requests; separate runtime resources preserve scan intervals and partial coverage, with explicit pause/restart/retry behavior.
- [Set overlay resource budgets and measurement gates](issues/15-set-overlay-resource-budgets.md) — bounded producer, broker and HTTP work target useful 1 GiB reference coverage; conservative age and expiry accompany explicit, still-unexecuted performance and correctness gates.
- [Define the cross-repo verification strategy](issues/11-define-verification-strategy.md) — independent corpus-driven unit, UDS and browser tests supplement controlled debug/release integration; exact-commit evidence, dedicated-host measurements and accessibility review gate delivery.
- [Assemble the cross-repo handoff](issues/12-assemble-cross-repo-handoff.md) — reviewed Volmap entry, CBRD-27398 local draft/develop-PR plan and cross-repo delivery/evidence ownership are ready for the next workflows; no implementation, measurement or publication is claimed.

## Not yet specified

None. All decision tickets are closed and the planning handoff is complete.
Implementation and release evidence remain downstream work, not map fog.

## Out of scope

- [Decide whether flush-transition events belong in this overlay](issues/17-decide-transition-changefeed-scope.md) — defer event capture, replay and causal timelines to a separate effort; retain sampled flushing state without inferring event timing, counts, cause or durability from scan differences.

- [Decide whether AOUT history belongs in this overlay](issues/16-decide-aout-overlay-scope.md) — defer AOUT collection and history visualization to a separate effort: the inspected engine disables AOUT, and its retained markers are neither timestamped eviction events nor proof of current nonresidency; no engine re-enablement is included.

- Implementing the feature inside this map; the map hands off to `/to-spec` (volmap) and the JIRA/PR flow (CUBRID).
- Making volmap a transaction-visibility or committed-state tool; the overlay explains engine residency, never commit state.
- Any engine write or repair path; the inspector never mutates database state.
- TCP or remote exposure of the inspector channel; remote use goes through SSH forwarding (design reference).
- Always-on production monitoring; the default-off parameter and debug gates stand.
- Shared-memory raw-struct export and raw process-memory traversal (rejected by the design reference).
- A per-BCB `SHOW` statement variant — deferred out of v1 by [ticket 03](issues/03-choose-transport-channel.md); the JIRA issue may cite it as future work, and it remains the fallback position if upstream review resists the socket.
- Resident page inspection — protected page-image capture, main-volume and DWB evidence, TDE normalization, digests, capture bracketing, consistency classification, and the independent Volmap disk cross-check — moves to a later Wayfinder map by [Choose the consistency-inspection boundary](issues/13-choose-consistency-inspection-boundary.md); none of it is required by the state-only overlay handoff.
- [Decide the TUI parity treatment for the overlay](issues/07-decide-tui-parity-treatment.md) — superseded by ADR-0006 and the focused TUI specification: runtime observations remain web-only, so an eighth TUI channel, terminal cadence semantics, and runtime-specific parity gates are outside this effort rather than latent parity debt.
