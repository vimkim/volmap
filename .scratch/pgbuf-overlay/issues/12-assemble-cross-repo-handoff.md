Type: task
Status: open
Blocked by: 02, 03, 04, 05, 06, 07, 08, 09, 10, 11

# Assemble the cross-repo handoff

## Question

Collapse the map's decisions into the two implementation entry points:

1. Volmap side — a spec entering `/to-spec` → `/to-tickets` → `/implement`: overlay module, endpoint, rendering, vocabulary/ADR edits, tests, README amendments.
2. CUBRID side — a JIRA issue draft plus develop-PR plan entering the `cubrid-jira-issue-write` / `cubrid-pr-create` flow: system parameter, scan/serializer, socket, gates, tests, with the CBRD-26325 instrumentation proposal cited as prior art.
3. A delivery order across both — recommended first slice: state-only residency + latch + dirty over the chosen transport, one overlay channel on the mosaic — and the explicit statement of which later phases (digests, DWB, TDE, changefeed) stay out of the first slice.
4. Distinguish closed decisions from release gates (performance measurements, upstream review outcomes, org approval for the new socket).

## Comments

## Answer
