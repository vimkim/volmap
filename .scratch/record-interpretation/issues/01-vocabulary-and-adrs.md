# 01 — Amend disclosure vocabulary; write ADR-0001 and ADR-0002

Blocked by: —
Blocks: 02, 03, 04, 05, 06

## Goal

Make the record-interpretation feature *legal* in volmap's domain model and
record the two hard-to-reverse decisions. Documentation only; no code.

## Work

1. `CONTEXT.md` amendments:
   - **Application payload** (line ~179): replace "Version one never displays
     application payloads" with the explicit-target disclosure rule: decoded,
     typed attribute values are retained in the graph and displayed only for
     records the operator explicitly deep-inspected; raw/undecodable bytes
     remain withheld everywhere.
   - **Deep inspection** (line ~167): add slots/records to its scope; rework
     the "without exposing application payloads" clause to reference the new
     disclosure term.
   - New terms: **Record interpretation** (revision-scoped decoded evidence,
     distinct from physical facts), **Class representation** (schema evidence
     entity keyed (class OID, reprid)), **Explicit-target disclosure** (the
     new line between structural facts and user values). Follow the existing
     entry format including the `_Avoid_:` line.
2. `docs/adr/0001-explicit-target-disclosure.md`: context (old rule, why it
   existed), decision (the rule above; retention AND display; no hex
   fallback), consequences (exports carry values for inspected records; tests
   pinning "retains_no_payload_bytes" for OOS chains stay valid and untouched).
3. `docs/adr/0002-classrepr-from-class-record.md`: interpretation resolves
   through the class object's own heap record, not the system catalog
   (catalog DISK_REPR = optimizer statistics; its extendible hash is dead
   code — cite `docs/record-interpretation-research.md` §3.1/§3.5).
   Consequences: `(volid, sectid)` cache key (heaps span volumes, §5.3);
   page-granularity enrichment (one click interprets the page's home records,
   one revision advance).

There is no `docs/adr/` today (repo has zero ADRs) — create it.

## Acceptance

- CONTEXT.md self-consistent: no remaining term forbids displaying decoded
  values for explicit targets; adapter/never-decode and bytes-withheld rules
  intact.
- Both ADRs cite the research doc and the prototype branch as primary sources.
