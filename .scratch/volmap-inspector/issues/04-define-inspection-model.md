Type: grilling
Status: resolved
Blocked by: 02, 15

# Define the canonical inspection model and evidence levels

## Question

What single presentation-independent model should CLI, JSON, TUI, exported HTML, and the web service share? Decide the entities and relationships from database snapshot through volume, sector, page, owning file, slotted-page slot entry, recognized record metadata, and OOS value chain; distinguish observed bytes, source-backed interpretation, derived summaries, anomalies, unreadable data, and unsupported formats; define fast versus deep inspection; and specify which identities and links remain stable across adapters. Use the project glossary, and revise it when this decision sharpens or replaces a term.

## Comments

- Source-backed modeling constraints: volume/sector/page containment is hierarchical, but file ownership crosses that hierarchy; heap OOS inline stubs link slot records to head OOS OIDs; chunk records link onward and may be corrupt or cyclic; diagnostics and evidence may attach at every level. Therefore a recursive presentation tree cannot be the sole lossless model.
- The model decision is intentionally separate from ticket 05's in-memory/index/cache architecture. Ticket 04 defines logical entities, identities, relationships, evidence, and inspection completeness; later adapters may materialize or query that model without changing its semantics.
- Decision tree: canonical model shape → entity set and relationships → stable identities → evidence/diagnostic representation → fast/deep completeness → cross-adapter projection and navigation invariants.
- Pinned physical identity facts for later rounds: volume=`VOLID`; sector=`(VOLID, SECTID)`; page=`VPID(VOLID, PAGEID)`; file=`VFID(VOLID, FILEID)`; slotted record/OOS chunk=`OID(VOLID, PAGEID, SLOTID)`; an OOS value chain is addressed by its head OOS OID. All must be scoped by the inspected database snapshot to prevent identities from different inputs being confused. Recognized record metadata is decoded detail attached to the physical slot record unless the human model decision promotes it to a separately addressable entity.
- Human decision tree (answers intentionally pending): (Q1) normalized graph versus nested/adapter-specific models; then (Q2) canonical entity granularity; (Q3) snapshot-scoped stable identity and link syntax; (Q4) evidence/provenance representation; (Q5) anomaly, unreadable, unsupported, and incomplete distinctions; (Q6) fast versus deep inspection guarantees; and finally (Q7) projection/navigation invariants shared by CLI, JSON, TUI, HTML export, and web.
- Human decision Q1 (2026-08-18): CLI, JSON, TUI, HTML export, and web share one normalized, snapshot-scoped inspection graph. Nested views are adapter projections, not alternate models; materialization and caching remain ticket 05 concerns.
- Human decision Q2 (2026-08-18): canonical addressable entities are database snapshot, volume, sector, file, page, slot entry, and OOS value chain. `Page` and `Slot entry` keep one physical identity and carry recognized page-type/record-type detail variants. An OOS chunk record is an OOS-detail slot; an OOS value chain is a distinct logical entity keyed by its head OOS OID because it may span slots and pages. Containment, file allocation/ownership, heap-record-to-chain references, and ordered chain chunks are explicit graph relationships.
- Human decision Q3 (2026-08-18): every entity identity is scoped by one opaque `SnapshotId`, which is unique to an inspection artifact and preserved unchanged across all adapters and exports. Physical keys are `VOLID`, `(VOLID,SECTID)`, `VFID`, `VPID`, and `OID`; an OOS chain key is its head OOS OID. A rescan creates a new `SnapshotId`. Relationships carry typed entity references rather than presentation URLs/selectors, and an invalid on-disk reference retains its intended physical key for diagnostics without inventing a valid target entity.
- Human decision Q4 (2026-08-18): facts carry traceable evidence rather than an entity-wide confidence score. `Observed` evidence records a bounded volume byte range and read outcome without exposing forbidden content; `Interpreted` evidence names the pinned format profile and validation rule that decoded observed ranges; `Derived` evidence names its input facts/entity references and derivation rule. Conflicts remain visible as diagnostics referencing their evidence.
- Human decision Q5 (2026-08-18): availability, inspection coverage, and diagnostics are orthogonal. Availability is `available`, `unreadable`, `unsupported`, or `encrypted-opaque`; coverage is `not-requested`, `partial`, or `complete` relative to the selected inspection mode. A structured diagnostic carries code, severity, affected entity/reference, message, and evidence references. An anomaly is readable, supported evidence that violates an invariant or conflicts with other evidence; it never erases other valid facts.
- Human decision Q6 (2026-08-18): default fast inspection is shallow but complete across the validated snapshot, never sampled. It decodes volume headers/sector bitmaps and the authoritative file tracker, file headers, and allocation tables; creates every addressable volume, sector, page, and file; classifies every page; validates the plaintext envelope of system and allocated pages for physical type, TDE state, identity, and LSA integrity; and derives allocation summaries/anomalies. Page bodies, slots, records, and OOS chains remain `not-requested`.
- Human decision Q7 (2026-08-18): deep inspection enriches the existing graph for an explicit target set, preserving snapshot/entity identities. A selected page is optionally decrypted, validated, and decoded through its page-type detail; slotted pages gain slots, byte maps, record kinds, and supported metadata. Heap-record OOS references do not auto-traverse. Selecting an OOS reference/chain follows bounded, validated chunk links and adds ordered cross-page chunk-slot relationships. Failures remain local and payload is never exposed.
- Human decision Q8 (2026-08-18): every projection declares `SnapshotId`, inspection revision, and coverage summary. Deep enrichment advances revision without changing entity identities; HTML freezes one revision while interactive adapters may observe later ones. Adapters may filter/sort/paginate/format but never parse, derive, classify, or reinterpret. All facts, evidence, relationships, availability, coverage, and diagnostics come from the graph; typed references round-trip and broken/unsupported targets remain visible. Ordering is canonical only when semantically meaningful.
- Human decision Q9 (2026-08-18): the graph preserves every evidence-backed relationship claim and never normalizes a conflict away. Physical VPID containment is exact; file tables yield zero, one, or multiple page-ownership claims, with ownership resolved only for exactly one validated claim. Missing/invalid targets remain unresolved typed references. OOS chains become complete only after every link/index/length/type/termination invariant validates; broken or cyclic chains retain valid partial chunks/links and diagnostics without fabricated repair.
- Human decision Q10 (2026-08-18): the database snapshot carries an input-fingerprint manifest. Any detected size, identity, timestamp, or page-LSA change is a snapshot-level fatal diagnostic that invalidates the current inspection revision, stops enrichment, makes retained facts diagnostic-only, and produces a nonzero CLI/JSON result plus prominent interactive/export warnings. Continuing requires a stable copy and a new `SnapshotId`; ticket 05 may optimize checks but cannot weaken this semantic contract.
- Human decision Q11 (2026-08-18): replace overloaded `Page classification` with allocation-only `Page allocation class`: `system-metadata`, `unreserved`, `reserved-unallocated`, or `allocated`. Physical page type, ownership claims/resolution, availability, TDE state, coverage, and diagnostics remain independent dimensions.

## Answer

All modes share one normalized, snapshot-scoped **inspection graph** behind the presentation seam. CLI, stable JSON, TUI, HTML export, and web are adapters over that model: they may select, filter, sort, paginate, abbreviate, color, and lay out the graph, but they never read database bytes, decode formats, derive summaries, classify entities, or reinterpret diagnostics. Ticket 05 may choose eager/lazy materialization, indexes, and caches without changing these semantics.

### Entities and relationships

The addressable entity kinds are:

- **Database snapshot** — root input and identity scope; carries format-profile identity, input-fingerprint manifest, inspection revision, consistency, and aggregate coverage.
- **Volume** — keyed by `VOLID`.
- **Sector** — keyed by `(VOLID, SECTID)`.
- **File** — keyed by `VFID`; the logical allocation owner described by the file tracker/header.
- **Page** — keyed by `VPID`; carries physical page type and an optional recognized page-type detail variant without changing identity.
- **Slot entry** — keyed by `OID`; carries slot allocation/record kind and an optional recognized record-detail variant. An OOS chunk record is an OOS-detail slot, not a second physical entity.
- **OOS value chain** — keyed by its head OOS OID; the logical cross-page value container whose ordered members reference OOS chunk slots.

Explicit relationships are snapshot containment of volumes; volume containment of sectors/pages; sector containment of pages; file sector/page allocation claims; uniquely resolved page ownership; page containment of slots; heap-slot OOS references; and ordered OOS-chain chunk-slot membership. A heap slot may reference zero or many chains, one OOS page may contain chunks from many chains, and one chain may span many pages.

Physical containment is exact, but allocation/link evidence is not normalized away. Zero ownership claims yields no owner, exactly one validated claim yields resolved ownership, and multiple claims remain a conflicting set with an anomaly. Missing or invalid targets remain unresolved typed references carrying the intended physical key. An OOS chain is complete only when all chunk links, indices, lengths, expected types, accumulated size, and termination validate; broken/cyclic chains retain valid partial chunks and claims without fabricated repair.

Page allocation topology is represented only by **page allocation class**: `system-metadata`, `unreserved`, `reserved-unallocated`, or `allocated`. Physical page type, ownership, availability, TDE state, coverage, and diagnostics are independent dimensions.

### Identity, revisions, and references

Every identity is qualified by an opaque `SnapshotId` unique to one inspection artifact:

```text
snapshot   SnapshotId
volume     SnapshotId + VOLID
sector     SnapshotId + VOLID + SECTID
file       SnapshotId + VFID
page       SnapshotId + VPID
slot       SnapshotId + OID
OOS chain  SnapshotId + head OOS OID
```

A rescan creates a new `SnapshotId`; the tool does not claim two scans observed the same point in time. Deep enrichment advances a monotonically increasing inspection revision while preserving every identity. Each projection declares snapshot ID, revision, and coverage summary. HTML freezes one revision; interactive adapters may observe later revisions. Relationships contain typed entity references, never URLs, paths, or CLI selectors; adapters translate references into navigation and must preserve round-trip identity.

### Evidence and outcomes

Facts carry provenance at fact granularity:

- **Observed evidence** — bounded volume byte range plus successful/failed read outcome; forbidden bytes are never exposed.
- **Interpreted evidence** — typed fact decoded from observed ranges under the pinned format profile and a named validation rule.
- **Derived evidence** — summary or relationship computed from named facts/entity references under a named derivation rule.

Availability, coverage, and diagnostics remain orthogonal:

- availability: `available`, `unreadable`, `unsupported`, or `encrypted-opaque`;
- coverage relative to the requested mode: `not-requested`, `partial`, or `complete`;
- diagnostic: stable code, severity, affected entity/reference, message, and evidence references.

An anomaly is a diagnostic for readable, supported evidence that violates an invariant or conflicts with other evidence. It never deletes otherwise valid facts. A fast-skipped body is `available + not-requested`, a recognized valid page without a v1 body decoder is `unsupported`, a failed/truncated read is `unreadable`, and a valid encrypted page without keys is `encrypted-opaque`.

### Fast and deep inspection

Default **fast inspection** is snapshot-wide, shallow, complete, and unsampled. It validates the snapshot/profile; discovers all validated volumes; decodes all volume headers/sector bitmaps and the authoritative file tracker, file headers, and allocation tables; creates every addressable volume, sector, page, and file; assigns page allocation class; validates plaintext envelopes for system/allocated pages; records physical type, TDE state, identity, and LSA integrity; and derives allocation summaries and cross-source anomalies. Page bodies, slots, record details, and OOS chains remain `not-requested`.

**Deep inspection** enriches an explicit target set in the same graph. A selected page is optionally decrypted under ticket 15, structurally validated, and decoded through its recognized page-type detail. Slotted pages gain headers, slot entries, byte maps, record kinds, and supported page-specific metadata. Heap records expose OOS references but do not automatically traverse them. Selecting an OOS reference/chain follows bounded validated links with cycle, step, and length guards and adds ordered chunk-slot relationships. Failures remain local and preserve the complete fast graph. No mode exposes application payload bytes/values, ciphertext, or TDE secrets.

### Snapshot invalidation

The snapshot carries an input-fingerprint manifest. Any detected volume size, identity, timestamp, or page-LSA change raises a snapshot-level fatal diagnostic, invalidates the current revision, stops further enrichment, and makes retained facts diagnostic-only. CLI/JSON return a nonzero outcome and interactive/export adapters display prominent invalidation. Continued inspection requires a stable stopped database, immutable snapshot, or copied volume set and a new `SnapshotId`. Ticket 05 may optimize verification timing but cannot weaken this contract.
