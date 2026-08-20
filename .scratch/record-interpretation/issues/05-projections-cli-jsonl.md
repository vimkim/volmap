# 05 — projections + CLI/JSONL adapters

Blocked by: 04
Blocks: 06

## Goal

Every adapter projects the same normalized interpretation facts; CLI and
JSONL ship in this ticket (D13).

## Work

- `projection.rs`: `ClassRepresentationProjection` (schema: attributes with
  name/type/domain) and `RecordInterpretationProjection` with per-attribute
  tagged three-state enums — copy the `ClassNameProjection` shape
  (`projection.rs:222`: NotApplicable/Resolved/Unresolved{reason}); never an
  omitted field or empty string. Undecodable attrs carry type name + offset +
  length + reason, **no bytes** (D3/D12).
- `DataProjection::InspectSlot` (`projection.rs:110`): add interpretation +
  class-representation fields.
- CLI `volmap inspect slot:v:p:s` (`cli.rs:570`): auto-enrich now also runs
  the page interpretation (after existing bigone/relocation logic); human
  output renders name = value lines + unresolved reasons; JSONL
  (`write_jsonl:865`) gains `record-interpretation` and
  `class-representation` typed records beside `slot`.
- Confirm HTML export (`export.rs` `complete_document:97`) picks the new
  facts up via the ticket-03/04 accessors — data present in the frozen
  document even though the exported UI doesn't render slots yet (backlog B2).

## Acceptance

- `volmap inspect slot:…` on a demodb fixture prints interpreted values;
  JSONL round-trips them; snapshot/golden tests updated deliberately (no
  accidental schema drift — JSONL is a versioned surface, match existing
  test conventions in `tests/`).
