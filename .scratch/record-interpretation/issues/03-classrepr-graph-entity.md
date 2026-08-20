# 03 — inspection module: class representation as a graph entity

Blocked by: 02
Blocks: 04

## Goal

`ClassRepresentationFact` becomes revision-scoped evidence in the inspection
graph, keyed `(class_oid, reprid)`, with a `(volid, sector) → class_oid`
internal index for cache hits (D7).

## Work

- `SessionData` (`inspection.rs:986`): add
  `class_representations: BTreeMap<(Oid, i32), ClassRepresentationFact>` and
  the sector index. Follow the OID-keyed precedent (`oos_chains`,
  `relocation_edges`).
- Resolution path: given a heap page → slot 0 class OID (existing
  `decode_heap_page` facts) → read the class record (follow REC_RELOCATION,
  bounded hops + cycle detection — mirror `resolve_class_name`
  `inspection.rs:2085`) → `format/classrep.rs` parse → publish.
- `publish_class_representation`: clone SessionData, bump revision, refresh a
  new `"class-representations"` coverage facet (mirror `refresh_oos_coverage`
  `inspection.rs:4605`), reclassify outcome. Idempotent: existing identical
  key → `self.clone()`, no bump. `verify_unchanged` brackets around all source
  reads (mirror `enrich_page` `:2139`/`:2310`), degrade to
  `invalidated_revision()` on movement.
- Failure evidence: unresolvable class record (malformed, TDE-opaque,
  NULL-class heap per D2) publishes a durable diagnostic (mirror
  `page_decode_failure` `:4131`), not a dropped request.
- `GraphView` accessors `class_representation(key)` /
  `class_representations()` (mirror `:3919`/`:3932`) — required so exports
  carry the facts.

## Acceptance

- Integration test: enrich a demodb-fixture heap page's class → fact present,
  revision advanced exactly once, second enrichment no-ops; sector-index hit
  on a second page of the same sector skips the class-record re-read.
- Invalidation test: mutate the backing fixture copy between reads → snapshot
  invalidates, no partial fact.
