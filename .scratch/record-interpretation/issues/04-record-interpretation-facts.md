# 04 — inspection module: record interpretation facts (page granularity)

Blocked by: 03
Blocks: 05

## Goal

`GraphView::enrich_record_page(vpid, policy, cancel)`: interpret **all home
records of one heap page** as one enrichment (D8) and store per-record facts.

## Work

- `SessionData`: `record_interpretations: BTreeMap<Oid, RecordInterpretationFact>`
  (key = record OID). Fact: class ref `(class_oid, reprid)` entity reference,
  per-attribute three-state values (Decoded/Null/Unresolved{reason}) + OOS
  stub arm (entity reference to the chain head OID + full length, D4).
- Scope per D1/D2: slots ≥ 1, HOME/NEWHOME interpret; RELOCATION publishes
  the edge (reuse/extend `publish_relocation_edge` `inspection.rs:4048`) and
  interprets the one-hop target; BIGONE records the forward VPID only;
  tombstones skipped. NULL-class page → whole-page degradation diagnostic.
- One revision advance per page enrichment (batch publish); idempotent
  re-enrichment no-ops; `verify_unchanged` brackets; class-level failure →
  durable diagnostic + structural view intact (D10).
- Old representations: request reprid from ticket-03's resolution path, which
  falls back to the old-repr walk (D9); unknown reprid → per-record
  unresolved reason naming both reprids.
- Coverage facet `"record-interpretations"`; `GraphView` accessors
  `record_interpretation(oid)` / `record_interpretations()`.

## Acceptance

- Integration tests on fixtures: page of `public.game` → every home record
  interpreted with golden values; relocation fixture → edge + target both
  present; OOS fixture → stub reference with correct OID/length; TDE-opaque
  and NULL-class pages → diagnostics, no panic, structural facts intact.
- Revision arithmetic: N clicks on same page = 1 advance total.
