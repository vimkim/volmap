Type: task
Status: resolved
Blocked by: 02, 03, 04, 05, 06, 07, 08, 09, 10, 11, 15, 16, 17

# Assemble the cross-repo handoff

## Question

Collapse the map's decisions into the two implementation entry points:

1. Volmap side — a spec entering `/to-spec` → `/to-tickets` → `/implement`: overlay module, endpoint, rendering, vocabulary/ADR edits, tests, README amendments.
2. CUBRID side — a JIRA issue draft plus develop-PR plan entering the `cubrid-jira-issue-write` / `cubrid-pr-create` flow: system parameter, scan/serializer, socket, gates, tests, with the CBRD-26325 instrumentation proposal cited as prior art.
3. A delivery order across both for the state-only overlay — residency + latch + dirty over the chosen transport, then the chosen visual channel — and an explicit boundary statement that resident page inspection (protected capture, digests, DWB, TDE normalization, and consistency classification) belongs to the later map chosen by ticket 13 rather than this handoff.
4. Distinguish closed decisions from release gates (performance measurements, upstream review outcomes, org approval for the new socket).

## Comments

- Handoff assembly started. All listed dependencies are resolved. Publication
  is outside this planning-only invocation: no implementation, commit, push,
  JIRA update or PR creation is authorized by this ticket.
- The user selected CBRD-27398. Live lookup confirmed an Open Sub-task under
  CBRD-27193 with an empty fetched description. The instrumentation proposal
  remains prior art, not the target issue. The keyed local draft and
  develop-PR plan do not imply publication authorization in this map.

- [Define the cross-repo verification strategy](11-define-verification-strategy.md)
  is resolved. Assign concrete testcase paths and implementation owners for
  its coverage matrix, companion external shell case and evidence manifest.
  Required local/release gates must not be described as existing hosted CI.
  The AOUT and flush-transition scope decisions are now closed exclusions.
- Carry the exclusion from
  [Decide whether flush-transition events belong in this overlay](17-decide-transition-changefeed-scope.md)
  into both handoffs: sampled flushing fields remain, but event hooks,
  changefeed endpoints, history/replay and causal timelines are deferred.
  Preserve the existing wire, budgets and state-only verification gates.
- Carry the scope exclusion from
  [Decide whether AOUT history belongs in this overlay](16-decide-aout-overlay-scope.md)
  into both handoffs: no AOUT collection, wire fields, history visualization
  or engine re-enablement. Its dependency edge remains as decision history,
  but the resolved ticket no longer blocks delivery planning.

- Carry the accepted limits and unexecuted measurement gates from
  [Set overlay resource budgets and measurement gates](15-set-overlay-resource-budgets.md)
  into both handoffs. A resolved budget decision is not a passed performance
  test or implementation authorization within this map.

## Answer

Resolved as the final planning-only task. The user selected CBRD-27398; no
issue creation, description update, commit, push or PR publication occurred.

The [handoff index](../handoff/README.md) links the completed artifacts:

- [Volmap implementation entry](../handoff/volmap-entry.md): private broker,
  normalized HTTP, rendering, existing glossary/ADR boundaries, tests and
  README ownership, ready to consolidate through `/to-spec`.
- [CUBRID entry and develop-PR plan](../handoff/cubrid-entry.md), paired with
  the reviewed Korean local draft
  [CBRD-27398-pgbuf-overlay_cd593bc_codex.md](/home/vimkim/gh/my-cubrid-jira/issues/CBRD-27398-pgbuf-overlay_cd593bc_codex.md):
  parameter, semantic producer, socket/security, branch alignment, test paths
  and CBRD-26325 prior art, ready for the later issue/PR workflow.
- [Delivery plan](../handoff/delivery-plan.md): contract/corpus, bounded
  producer, consumer boundary, visual projection, integration/port and
  release readiness, with concrete role/path assignments and evidence gates.

The mandatory draft review identified and corrected four clarity gaps:
evaluated-only absence, browser scope/epoch revocation, develop debug/release
coverage and unexplained terminology. Re-review approved the draft and found
no material conflicts with the three handoff documents. Document link,
whitespace, English top-level heading and portability checks passed.

All accepted source contracts remain linked and unchanged in meaning. The
handoff explicitly excludes resident capture/consistency work, runtime TUI
parity, AOUT history and flush events. The planned implementation artifacts
and tests do not yet exist; quantitative and manual gates have not run.
Upstream/socket approval and publication remain downstream gates, not claims
made by closing this map. No further design ticket or un-ticketed fog remains.
