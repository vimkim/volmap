# 01 — Input fingerprint manifest and source mode

Blocks: 02, 03

- Add `source::InputFingerprint` (ordered declared volume id + `FileStamp`,
  plus input kind) and `source::fingerprint(&InputSpec)`, which stats volume
  paths after re-reading the small manifest so volume-set membership changes
  count as a change. No page reads.
- Add `SourceSet::fingerprint()` returning the manifest observed at discovery.
- Add `SourceMode { Immutable, Live }` to `OpenRequest`.
- Widen `SnapshotValidity` with `Torn` and `Superseded`; update
  `projection::validity_name` and every match.
- In `Inspection::open`, a mid-scan manifest change is `Invalidated` +
  `snapshot.modified` (fatal) under `Immutable` and `Torn` +
  `snapshot.torn_read` (warning) under `Live`.
- In every enrichment entry point, a manifest change under `Live` marks the
  produced revision `Superseded` + `snapshot.source_advanced` (warning) and
  keeps the facts, instead of returning `invalidated_revision()`.

Done when: unit tests cover fingerprint equality across a touch, a size change,
and an added volume; and both modes are asserted for a mid-scan change.
