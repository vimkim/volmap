# 02 — format layer: class-representation parser + attribute value decoder

Blocked by: 01
Blocks: 03, 04

## Goal

Pure byte-level decoding in `src/format/` (no inspection-module state):

1. `format/classrep.rs`: parse a class object's heap record into a
   `ClassRepresentationFact`-shaped struct: class name, rep id, n_fixed,
   n_variable, fixed_length, attributes (id, name, DB_TYPE, domain
   {precision, scale, codeset, collation}, is_fixed, location, position).
   - Current representation: port of `or_get_current_representation` — the
     prototype's `parse_class_record` (branch `prototype/record-interpretation`,
     `prototype-record-interp/src/main.rs`) is a working reference; research
     doc §3.5 has the cited constants (ORC_* enums, substructure-set layout).
   - **Old representations (D9)**: when target reprid ≠ current, walk the
     `ORC_REPRESENTATIONS_INDEX` (=2) substructure set comparing
     `ORC_REP_ID_OFFSET`; research §3.5. Attribute names for old reprs come
     from rep_attribute substructures (ORC_REPATT_* constants).
   - Class records always use 4-byte offsets; validate, don't assume.
2. Record value decoding against a parsed representation: per-attribute
   three-state result (Decoded(value) / Null / Undecodable{reason}) plus the
   OOS-stub arm (OID + length). Types per SPEC D3; the traps list in SPEC
   "Layout facts" is binding (24-bit reprid mask, `!0x3` var-entry mask, CHAR
   and NUMERIC are variable-region, varchar LZ4 prefix, NUMERIC 3-byte
   header). LZ4: reuse or add a vetted dependency consistent with
   `deny.toml`/`about.toml` (prototype used `lz4_flex`).

## Conventions (binding)

- `DecodeError` + dotted rule strings for format violations (e.g.
  `classrep.attset.count`); human-readable `&'static str` reasons only for
  text that reaches users verbatim (mirror the class-name path split —
  `inspection.rs:2090` vs `format/` discipline).
- No panics, no allocation-on-error, bounds-check everything: these bytes are
  hostile. Match the validation style of `format/slotted.rs`.
- Decoded string values respect the domain codeset; v1 may render via lossy
  UTF-8 but must carry the codeset in the fact.

## Acceptance

- Unit tests against `fixtures/` volumes (same style as `tests/heap_pages.rs`,
  `tests/catalog_pages.rs`): parse `public.game`, `public.stadium`
  (NUMERIC(10,2)), `public.olympic` (LZ4 varchar), a system class, and the
  OOS-stub table; golden expected values from the prototype's verified output.
- Fuzz entry (mirror `fuzz/fuzz_targets/slotted_records.rs`) for the classrep
  parser: arbitrary bytes never panic.
