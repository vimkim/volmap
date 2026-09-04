# Volmap Inspector

Volmap Inspector is a read-only offline explorer of CUBRID volume allocation and page structure. This glossary separates physical storage facts from interpretations presented by its CLI, TUI, and web viewer.

## Inspection model

**Inspection graph**:
The normalized, snapshot-scoped set of storage entities and explicit relationships that every CLI, JSON, TUI, HTML, and web view projects. Its meaning is independent of how entities are scanned, materialized, indexed, or cached.
_Avoid_: Presentation tree, adapter model

**Inspection adapter**:
A CLI-human, JSON/JSONL, TUI, HTML, or web projection of the shared inspection module's normalized query results. An adapter never reads or parses volume bytes and never invents adapter-specific storage facts.
_Avoid_: Parser frontend, independent inspector

**Terminal inspection flow**:
The TUI's focused Volume → Sector → Page path, with record interpretation shown as Page-local detail. It summarizes occupancy at higher levels and performs bounded deep inspection only for the active Page or selected record.
_Avoid_: Terminal interaction parity, Atlas trail, web mirror

**Inspection revision**:
A monotonically advancing version of one inspection graph as explicit deep-inspection targets add evidence and details. Revisions preserve the snapshot and entity identities; an export freezes one revision.
_Avoid_: Database version, schema version

**Database snapshot**:
The read-only set of CUBRID volumes inspected together as one stable input; it scopes every other inspection-graph identity.
_Avoid_: Live database, scan run

**Invalidated snapshot**:
A database snapshot whose input-fingerprint manifest no longer matches observed volumes. Its retained facts are diagnostic evidence only, and it cannot accept further inspection enrichment.
_Avoid_: Partial snapshot, stale cache

**Entity reference**:
A typed, snapshot-scoped identity used by one inspection entity to name another. Adapters may render it as navigation, but URLs, paths, and CLI selectors are not part of the reference.
_Avoid_: Link URL, pointer

**Entity selector**:
An adapter-specific textual address used to request one entity in the snapshot being inspected. A selector is parsed into a typed identity for a request but is never stored as an entity reference or graph identity.
_Avoid_: Entity reference, URL, path

**Unresolved entity reference**:
An on-disk physical identity whose target is missing, invalid, or unavailable. The intended identity remains visible for evidence and diagnostics without creating a target entity.
_Avoid_: Broken URL, null entity

**Relationship claim**:
An evidence-backed on-disk assertion that entities are related. A missing target, type mismatch, cycle, overlap, or competing claim may prevent it from becoming a valid semantic relationship without erasing the claim.
_Avoid_: Resolved relationship, repaired link

**Observed evidence**:
A bounded volume byte range and the outcome of attempting to read it. It identifies source bytes without exposing application payload, ciphertext, or key material.
_Avoid_: Raw value, confidence

**Interpreted evidence**:
A typed fact decoded from observed evidence under the pinned format profile and a named validation rule.
_Avoid_: Parsed guess

**Derived evidence**:
A summary or relationship calculated from named interpreted facts or entity references under a named derivation rule.
_Avoid_: Observed value

**Availability**:
Whether requested evidence can be interpreted as `available`, `unreadable`, `unsupported`, or `encrypted-opaque`. It does not describe how much inspection was requested or completed.
_Avoid_: Status, validity

**Inspection coverage**:
Whether detail promised by the selected inspection mode is `not-requested`, `partial`, or `complete` for an entity.
_Avoid_: Availability, scan status

**Coverage ledger**:
The evidence-backed progress record for one requested inspection facet, including evaluated counts, only trusted totals, its stopped boundary and reason, and a known or explicitly unknown remainder.
_Avoid_: Progress bar, estimated coverage

**Inspection outcome**:
The aggregate automation result, ordered `success`, `success-limited`, `findings`, `incomplete`, then `fatal`. It summarizes but never replaces diagnostic severity, availability, coverage ledgers, or snapshot validity.
_Avoid_: Maximum severity, HTTP status

**Diagnostic**:
An evidence-backed finding with a stable code, severity, affected entity or reference, and explanatory message.
_Avoid_: Status string, parser error

**Diagnostic code**:
A stable lowercase namespaced identifier for one documented finding rule. Released code semantics are never repurposed, and adapters never infer behavior from human message text.
_Avoid_: Error message, numeric errno

**Diagnostic occurrence**:
One snapshot-scoped instance of a diagnostic rule, identified independently of revision, severity, and message wording by its affected entities or references and canonical evidence or relationship locus.
_Avoid_: Log line, duplicate finding

**Diagnostic severity**:
The finding's consequence for inspection trust and completion: `info`, `warning`, `error`, or `fatal`. It is independent of availability, coverage, containment scope, and presentation emphasis.
_Avoid_: Log level, page status

**Containment impact**:
The validation boundary and dependent inspection facets stopped by a diagnostic occurrence, together with the independently valid scopes that remain usable.
_Avoid_: Severity, blast radius estimate

**Anomaly**:
A diagnostic raised when readable, supported evidence violates an invariant or conflicts with other evidence. An anomaly does not erase facts that remain valid.
_Avoid_: Unreadable data, unsupported format

**Validation boundary**:
A bounded structure or reference with named prerequisite checks. Failure blocks only interpretations and traversals that depend on that boundary; independently validated parent facts, prior chain prefixes, and sibling entities remain usable.
_Avoid_: Trust score, parser scope

**Validated prefix**:
The ordered members of a linked structure reached through consecutively valid boundaries before its first invalid, missing, cyclic, or budget-stopped link. It does not imply that the complete structure is valid.
_Avoid_: Recovered chain, complete prefix

**Operational budget**:
An explicit tool-resource ceiling distinct from an on-disk format limit. Reaching it is not corruption; it stops at a validation boundary, preserves a validated prefix, and makes the promised inspection scope partial.
_Avoid_: Format maximum, silent truncation

## Storage hierarchy

**File**:
A CUBRID logical allocation owner identified by a VFID and described by file-tracker and file-header metadata; it may allocate pages across physical sectors.
_Avoid_: Volume file, operating-system file

**Page**:
One physical 16,384-byte page identified by a VPID. Its physical page type selects an optional recognized page-detail variant without changing the page's identity.
_Avoid_: Page-type object

**Volume**:
A CUBRID volume file belonging to the inspected database snapshot.
_Avoid_: Disk, database file

**Sector**:
A fixed physical region of 64 consecutive pages in a volume.
_Avoid_: Block, extent

**Sector summary**:
The derived reservation, allocation, ownership, utilization, and anomaly counts for one sector.
_Avoid_: Sector status

**Page allocation class**:
The page's evidence-backed allocation topology: `system-metadata`, `unreserved`, `reserved-unallocated`, or `allocated`. Physical page type, ownership, availability, and diagnostics are separate dimensions.
_Avoid_: Page classification, page status

**Page ownership**:
The resolved file identity and logical file type for a page when exactly one validated file-allocation claim exists. Conflicting claims remain visible and do not produce a resolved owner; ownership is distinct from physical page type.
_Avoid_: Page kind

**Page ownership claim**:
One evidence-backed assertion from a file's allocation metadata that it owns a page. A page may have zero, one, or conflicting multiple claims.
_Avoid_: Page ownership

## Page inspection

**Page detail support**:
The versioned interpretation promise for a recognized physical page type: `semantic`, `structural-only`, or `opaque`. It defines product scope independently of a particular page's availability, inspection coverage, or validity.
_Avoid_: Page status, decoder success

**Slotted page**:
A CUBRID page whose records are addressed through a slot directory and occupy byte extents within the page body.
_Avoid_: Record page

**Slot entry**:
A slot-directory entry describing a record's slot identifier, byte offset, length, and record type, or describing an empty/deleted slot. Recognized record-type details attach to this identity rather than creating another physical record entity.
_Avoid_: Record pointer

**Page byte map**:
A physical visualization of the page header, occupied record extents, alignment waste or gaps, contiguous free area, and slot directory across the page's byte range.
_Avoid_: Page status map

**Fast inspection**:
The snapshot-wide, unsampled pass that completely establishes volume geometry, sector reservation, file allocation, page allocation class, and plaintext page-envelope facts without decoding page bodies.
_Avoid_: Sample scan, partial scan

**Deep inspection**:
Opt-in enrichment of selected pages, slots, records, or OOS value chains with validated body structure, slot allocation, page-type details, record interpretations, and bounded chain relationships. Application payloads surface only under explicit-target disclosure.
_Avoid_: Deep scan

**Record interpretation**:
Revision-scoped decoded evidence for an explicitly deep-inspected record: its attribute names, domains, and typed values resolved through a class representation. It is distinct from, and never replaces, the record's physical facts.
_Avoid_: Row dump, record view

**Stored attribute extent**:
The exact record-relative byte interval occupied by one interpreted attribute, independent of whether its value is decoded, NULL, withheld, or out of row. A variable NULL has a proven zero-width position rather than an invented byte interval, while a fixed NULL retains its fixed-storage extent.
_Avoid_: Value size, column width, payload range

**Attribute byte selection**:
The adapter-local choice of one interpreted attribute whose stored extent and related metadata anchors are emphasized across record and page byte maps. It is not an inspection entity, does not enter canonical navigation history, and survives a refresh only when record identity, representation, and attribute position still match.
_Avoid_: Column entity, attribute URL, selected value

**Byte-coordinate projection**:
A typed mapping of one stored extent into explicitly named record-relative, page-content, physical-page, and volume-file coordinate spaces. Format arithmetic and coordinate origins belong to the projection, never to an inspection adapter.
_Avoid_: Byte offset, frontend offset calculation, absolute offset

**Metadata anchor**:
A proven byte or bit location whose stored metadata determines an attribute's interpretation, such as a bound bit or variable-offset entry. It is presented separately from the attribute's primary stored extent.
_Avoid_: Attribute extent, inferred marker

**Class representation**:
A schema evidence entity keyed by (class OID, reprid) that names one representation's attributes and their typed domains, decoded from the class object's own heap record.
_Avoid_: Catalog representation, DISK_REPR

**TDE inspection state**:
The canonical visibility or failure classification for a page: `not-encrypted`, `decrypted`, `encrypted-opaque`, `key-error`, `decrypted-invalid`, or `invalid-flags`.
_Avoid_: Encryption status

**Encrypted opaque page**:
A page whose plaintext I/O envelope validly identifies AES or ARIA encryption but whose user region is intentionally not decrypted because no key file was supplied. It remains distinct from an unreadable or corrupt page.
_Avoid_: Encrypted error, unreadable encrypted page

**Application payload**:
Record bytes or decoded user values not required to describe physical allocation, record boundaries, record kind, or internal storage navigation. Explicit-target disclosure governs when any of it surfaces.
_Avoid_: Raw metadata

**Explicit-target disclosure**:
The disclosure rule separating structural facts from user values: decoded, typed attribute values are retained in the inspection graph and displayed only for records the operator explicitly deep-inspected, while raw or undecodable payload bytes remain withheld everywhere.
_Avoid_: Payload opt-in, selective disclosure

## OOS storage

**OOS page**:
A slotted page with physical page type `PAGE_OOS` in an OOS file.
_Avoid_: Overflow page

**OOS chunk record**:
One physical slotted-page record containing an OOS record header and a payload fragment.
_Avoid_: OOS page, OOS record

**OOS value chain**:
The logical storage object addressed by a head OOS OID whose validated chunk sequence should contain one complete serialized attribute value. Inspection may retain a partial or corrupt chain without treating it as complete.
_Avoid_: OOS chain page

## Distribution

**Live inspection session**:
One foreground `serve` process, its private cursor-integrity key, the snapshot generations it currently retains with their inspection revisions, cursors, and browser/API locations. Publishing a generation replaces the one on display; evicted generation state is discarded. The HTTP interface is unauthenticated; remote exposure requires an explicit IPv4 wildcard listener. All process state expires together when the process ends and is never reused as a persistent inspection index.
_Avoid_: Web deployment, daemon, saved report

**Source mode**:
Whether an input is read under the offline `immutable` contract or as one generation of a `live` follow. It selects the consequence of an input change, not how the input is read. `serve` follows by default; every other command is immutable.
_Avoid_: Read mode, online mode, live database

**Snapshot generation**:
One complete fast scan of a live input, numbered monotonically within a live inspection session, carrying its own database-snapshot identity and its own inspection-revision chain. Generations replace one another rather than revising one another, and deep-inspection enrichment never carries over between them.
_Avoid_: Snapshot refresh, reload, rescan revision

**Input fingerprint manifest**:
The ordered declared data-volume identities and file stamps observed for one generation, including volume-set membership so an added or removed volume counts as a change. Volumes declared with negative identifiers, which is to say the log and the manifests themselves, are not part of it.
_Avoid_: Checksum, volume digest

**Torn generation**:
A generation whose fingerprint manifest changed during its own scan. Its facts are usable evidence labelled internally inconsistent; it is not an invalidated snapshot, and another scan is scheduled.
_Avoid_: Corrupt snapshot, partial read, failed scan

**Superseded generation**:
A published generation whose fingerprint manifest no longer matches the input. Its facts stay exactly as observed and stay queryable while retained. Being superseded is a statement about currency, not about correctness, and is never reported as a failure.
_Avoid_: Stale snapshot, invalidated snapshot

**Live follow**:
The watcher behaviour that polls the input fingerprint manifest, debounces an observed change, re-reads the input, and publishes the result as the next generation. Only `serve` follows, and it watches only the data volumes.
_Avoid_: Auto-reload, live monitoring, in-place rescan, snapshot refresh

**Generation retention window**:
The bounded count of recent generations kept addressable so that a collection load finishes on the generation it started on. A cursor naming a generation past the window is answered as stale rather than as a forgery.
_Avoid_: History, cache, undo buffer

**Observed disk state**:
What Volmap reports: the bytes present in the data volume files at the moment they were read. This is not committed database state. A change committed to the log but not yet written to a data volume is invisible to inspection, and a page written before its transaction commits is visible to it, so the delay a reader notices is the engine flush cadence rather than anything Volmap controls. Live follow shortens the gap between a write reaching disk and the viewer showing it; it does not make the viewer a transaction-visibility tool.
_Avoid_: Database state, committed state, current data, what the database contains

**Runtime page observation**:
One optional, timestamped diagnostic reading about a physical page from a running system. It is independently captured per page, does not revise the inspection graph, and never implies a database-wide atomic view.
_Avoid_: Live page state, page status, snapshot generation

**Runtime observation overlay**:
An optional presentation layer over observed disk state that displays bounded runtime page observations without changing inspection facts. Pausing the display freezes adoption of both newer disk generations and newer runtime observations while still allowing their availability to be reported.
_Avoid_: Live follow, inspection revision, page status color

**Runtime capability state**:
The availability of one optional runtime observation source to the live viewer: `disabled`, `connecting`, `active`, `stale`, `unavailable`, `refused`, or `incompatible`. It never changes inspection validity, outcome, diagnostics, or coverage.
_Avoid_: Inspection outcome, diagnostic severity, connection error

**Observation freshness**:
The age of a runtime page observation relative to its requested sampling cadence. It is `fresh` through two expected intervals and `stale` afterward; freshness never implies source coherence or currentness at display time.
_Avoid_: Current state, valid observation, snapshot age

**Observation batch**:
A bounded request or response covering the selected page and pages in currently visible sectors. Its members retain their own capture times and do not form an atomic buffer-pool or operating-system snapshot.
_Avoid_: Runtime snapshot, buffer-pool snapshot, volume observation

**Observation coverage**:
The evaluated and requested page counts for one bounded observation scope, including any resource-stopped remainder. It describes an ephemeral overlay request and never changes Inspection coverage.
_Avoid_: Inspection coverage, silent sampling, buffer-pool coverage

**Page-buffer observation**:
A runtime page observation describing a cooperating `cub_server`'s semantic buffer-frame evidence, such as residency, fix or latch state, dirty state, and transition limitations. It is distinct from both the persistent page image and operating-system cache residency.
_Avoid_: Memory page, cached page, disk state

**Resident page inspection**:
An explicit, selected-page diagnostic capture from a cooperating `cub_server` that may add sanitized in-memory page structure and persistence-comparison evidence. It never loads a missing page and is distinct from lightweight page-buffer observation.
_Avoid_: Runtime page observation, memory dump, live enrichment

**Page image correspondence**:
Evidence that a resident page capture and an observed persistent page image have the same normalized content. Their structural geometry may be combined only when this correspondence is proven; otherwise each remains a separately labelled observation.
_Avoid_: Same VPID, current page, synchronized boolean

**Runtime attachment**:
The explicitly requested association between one live inspection session and one cooperating `cub_server`, proven by database and volume identity evidence and bound to one server incarnation. A matching database name alone never establishes it.
_Avoid_: Auto-discovery, server connection, database-name match

**Kernel-cache observation**:
A runtime page observation classifying how much of one physical CUBRID page was resident in the operating system's file page cache when queried: `fully-resident`, `partially-resident`, `not-resident`, or `unknown`. It is an ephemeral residency reading, not a durability or CUBRID buffer-pool claim.
_Avoid_: Cached page, buffer hit, durable page

**HTML inspection export**:
A bounded, self-contained offline file that freezes one inspection revision and contains only facts already committed to that revision. It is not connected to a live inspection session and cannot request missing deep detail.
_Avoid_: Live viewer, database dump, cached session

**Standalone executable**:
The single Linux x86-64 `volmap` binary, with no required runtime dependency on glibc, CUBRID libraries, installation assets, network services, or separately installed web assets. Optional runtime observation sources may be absent, refused, or unsupported without reducing offline inspection behavior.
_Avoid_: Portable installation

## Evidence governance

**Format profile**:
The explicit choice of one format authority for an entire inspection input set. It determines how ambiguous persistent bytes are interpreted and remains visible with the resulting facts.
_Avoid_: Auto-detected version, database version

**Format authority**:
The pinned CUBRID source revision and company-generated fixtures from which supported persistent layouts and invariants are established.
_Avoid_: Legacy implementation

**Recovered artifact**:
A legacy executable or reverse-engineering output obtained from another CUBRID employee and kept outside Volmap Inspector source and distribution.
_Avoid_: Reference implementation, source code

**Behavioral oracle**:
An optional, explicitly authorized black-box comparison that records normalized observable facts from a recovered artifact; it is never a format authority.
_Avoid_: Golden implementation, compatibility source
