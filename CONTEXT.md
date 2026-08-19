# Volmap Inspector

Volmap Inspector is a read-only offline explorer of CUBRID volume allocation and page structure. This glossary separates physical storage facts from interpretations presented by its CLI, TUI, and web viewer.

## Inspection model

**Inspection graph**:
The normalized, snapshot-scoped set of storage entities and explicit relationships that every CLI, JSON, TUI, HTML, and web view projects. Its meaning is independent of how entities are scanned, materialized, indexed, or cached.
_Avoid_: Presentation tree, adapter model

**Inspection adapter**:
A CLI-human, JSON/JSONL, TUI, HTML, or web projection of the shared inspection module's normalized query results. An adapter never reads or parses volume bytes and never invents adapter-specific storage facts.
_Avoid_: Parser frontend, independent inspector

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
Opt-in enrichment of selected pages or OOS value chains with validated body structure, slot allocation, page-type details, and bounded chain relationships without exposing application payloads.
_Avoid_: Deep scan

**TDE inspection state**:
The canonical visibility or failure classification for a page: `not-encrypted`, `decrypted`, `encrypted-opaque`, `key-error`, `decrypted-invalid`, or `invalid-flags`.
_Avoid_: Encryption status

**Encrypted opaque page**:
A page whose plaintext I/O envelope validly identifies AES or ARIA encryption but whose user region is intentionally not decrypted because no key file was supplied. It remains distinct from an unreadable or corrupt page.
_Avoid_: Encrypted error, unreadable encrypted page

**Application payload**:
Record bytes or decoded user values not required to describe physical allocation, record boundaries, record kind, or internal storage navigation. Version one never displays application payloads.
_Avoid_: Raw metadata

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
One foreground `serve` process, its private cursor-integrity key, immutable revision history, enrichment jobs, cursors, and browser/API locations. The HTTP interface is unauthenticated; remote exposure requires an explicit IPv4 wildcard listener. All process state expires together when the process ends and is never reused as a persistent inspection index.
_Avoid_: Web deployment, daemon, saved report

**HTML inspection export**:
A bounded, self-contained offline file that freezes one inspection revision and contains only facts already committed to that revision. It is not connected to a live inspection session and cannot request missing deep detail.
_Avoid_: Live viewer, database dump, cached session

**Standalone executable**:
The single Linux x86-64 `volmap` binary, with no runtime dependency on glibc, CUBRID libraries, installation assets, network services, or separately installed web assets.
_Avoid_: Portable installation

## Evidence governance

**Format authority**:
The pinned CUBRID source revision and company-generated fixtures from which supported persistent layouts and invariants are established.
_Avoid_: Legacy implementation

**Recovered artifact**:
A legacy executable or reverse-engineering output obtained from another CUBRID employee and kept outside Volmap Inspector source and distribution.
_Avoid_: Reference implementation, source code

**Behavioral oracle**:
An optional, explicitly authorized black-box comparison that records normalized observable facts from a recovered artifact; it is never a format authority.
_Avoid_: Golden implementation, compatibility source
