Type: grilling
Status: resolved
Blocked by: 05, 15

# Decide whether flush-transition events belong in this overlay

## Question

Does the design reference's flush-path transition changefeed belong in this
handoff or a later effort? Current scans cannot establish transition order
or causality. If included, decide event loss/order, capture and retention
semantics, gating and independent resource limits, explicitly revisiting
the accepted latest-scan-only architecture and affected verification gates.

## Comments

Graduated from the map's transition-changefeed fog after the resource-budget
resolution. The user-approved scope exclusion is recorded below.

Local planning evidence:

- `docs/live-page-buffer-inspection.md:110` treats the transition changefeed
  as a candidate bounded, non-blocking extension; a baseline is required and
  event-only reconstruction is invalid after late attachment or a gap.
  Its delivery order places changefeed last and optional (`:129`).
- [Define wire contract v1](05-define-wire-contract-v1.md) includes sampled
  dirty/flushing/async-flush-requested fields and a per-scan sequence, not an
  event sequence or complete history. A whole flush can occur between scans.
- [Define the volmap overlay architecture](10-define-volmap-overlay-architecture.md)
  retains bounded current evidence/latest offer rather than runtime history.
  Event capture would require additional semantics and independent overhead
  evidence, not merely another interpretation of the existing state fields.

## Answer

Closed as out of scope with the user's “all recommended” acceptance.

Defer flush-transition events to a separate effort while retaining accepted
sampled dirty, flushing and async-flush-requested state and its static
visual mark. Exclude event hooks, changefeed endpoints, event buffers/replay
and causal timelines. Do not infer flush start/end, duration, counts, cause
or durability from scan differences.

Future work must define event identity/order, loss reporting, capture points,
retention/backpressure, gating, overhead and verification separately. Existing
wire, broker budgets and state-only gates remain unchanged.

The future effort must also establish baseline/gap recovery rather than
reconstructing current state solely from events after late attachment or
loss. No event instrumentation, history capability or event-specific test
gate is required in this handoff. This is a scope exclusion, not a permanent
rejection; record it under Out of scope rather than Decisions so far.
