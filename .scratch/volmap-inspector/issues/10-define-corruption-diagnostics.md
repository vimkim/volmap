Type: grilling
Status: resolved
Blocked by: 02, 04

# Define corruption containment and diagnostic semantics

## Question

When volume, file-table, page, slot, or OOS-chain bytes violate the pinned format, exactly what remains trustworthy and what must stop? Define validation boundaries, diagnostic identities and severity, safe arithmetic and bounds rules, cycle and overlap detection, per-volume/page containment, incomplete-report markers, nonzero exit behavior, and UI/JSON representation. The result must operationalize the standing rule: continue only where boundaries remain independently trustworthy and never infer across an untrusted offset or length.

## Comments

### Source-backed constraints

- The pinned format report requires fail-closed parsing per structure while continuing elsewhere with explicit diagnostics. Counts, offsets, enum values, multiplication, and linked references are untrusted until validated; all physical and logical references must be range-checked and checked against the expected page/record type.
- Physical page identity and the duplicated leading/trailing LSA are independently checkable in the plaintext envelope. An invalid envelope forbids page-body interpretation, but does not by itself invalidate a separately bounded sibling page. There is no generic checksum in the pinned profile.
- Volume geometry, sector bitmaps, file-header/extensible-data accounting, slotted-page directories and record extents, and OOS chunk chains each have their own structural invariants. Linked structures require visited sets and deterministic step/length bounds; record extents and allocation claims require overlap/duplicate detection.
- TDE ciphertext is never page-structure input. Recognized opaque encryption is availability, not corruption; invalid flags or structurally invalid decrypted plaintext are corruption under the resolved ticket-15 contract.
- The resolved inspection model preserves every independently proven fact and conflicting claim, leaves invalid targets as unresolved typed references, and retains valid OOS-chain prefixes without fabricating missing links. Snapshot mutation is already a fatal invalidation of the whole inspection revision.

### Human decision frontier

1. Trust-domain hierarchy and propagation: which parent failure blocks which descendants, and when siblings remain independently inspectable.
2. Salvage granularity inside one damaged structure: whole-structure quarantine versus retaining independently validated fields, table prefixes, slots, or chain prefixes.
3. Cross-entity violations: cycle, overlap, duplicate ownership, type mismatch, missing target, and disagreement between redundant sources.
4. Diagnostic identity and deduplication: stable codes, occurrences, subjects, evidence ranges, and revision behavior.
5. Severity and inspection outcome: distinguish informational limitations, warnings, corruption, fatal invalidation, partial completion, and adapter exit behavior.
6. Safe execution limits: checked arithmetic, allocation limits, traversal budgets, and how resource exhaustion differs from on-disk corruption.
7. Incomplete-report semantics: explicit coverage denominators, stopped-boundary markers, suppressed descendants, and prohibition on extrapolation.
8. Projection contract: deterministic JSON fields/order and consistent CLI, TUI, HTML, and web summaries/navigation without message parsing.

- Human decision Q1 (2026-08-18): corruption follows hierarchical validation boundaries. A failed boundary blocks only facts and traversals that depend on it. Independently validated parent facts, prior linked-structure prefixes, and sibling entities remain trustworthy and inspectable. Invalid volume geometry quarantines that volume's dependent topology; an invalid file-table component stops that chain; an invalid page envelope blocks its body; an invalid slot-directory boundary blocks records addressed through it; and an invalid OOS link stops that chain. Snapshot mutation remains the already-decided whole-revision exception.
- Human decision Q2 (2026-08-18): salvage is dependency-directed within a damaged structure. A bounded read outcome and the invalid on-disk claim remain evidence, without exposing raw contents, but a fact is interpreted only after its own byte range and all prerequisite boundaries validate. Invalid container counts, widths, or extents are never clamped and yield no guessed children. After child layout validates, fixed-position children validate independently. Linked structures retain only their validated prefix and never skip a bad link. Overlapping records retain both slot claims but block record-detail decoding for the participants; unrelated records continue. Invalid aggregate counters remain visible beside independently derived exact counts and a conflict diagnostic.
- Human decision Q3 (2026-08-18): cross-entity corruption is claim-preserving and never self-repairing. Every bounded reference/allocation claim and its evidence remain visible. Missing or out-of-range targets stay unresolved without fabricated entities. Wrong-type/identity targets retain both endpoint facts but do not establish the claimed semantic relationship. A cycle-closing edge remains evidence while traversal stops before repetition. Duplicate ownership retains every claimant and resolves no winner. Redundant-source conflicts preserve both claims unless the pinned format explicitly names an authority. One logical finding relates all affected claims/entities so adapters can navigate from either side without duplicating it.
- Human decision Q4 (2026-08-18): diagnostics have a stable lowercase namespaced code whose released semantics are never repurposed and an opaque snapshot-scoped occurrence ID derived deterministically from `SnapshotId`, code, the canonical affected entity/reference set, canonical evidence/relationship locus, and only when necessary an occurrence ordinal. Revision, severity, and message text are not identity. Repeated discovery merges evidence/participants; separate offending entries remain separate; one cross-entity conflict is one occurrence. Rescans intentionally create new IDs. Proven findings are monotonic within the snapshot: enrichment may add evidence but never silently remove or reinterpret them.
- Human decision Q5 (2026-08-18): diagnostic severity measures inspection consequence and is exactly `info`, `warning`, `error`, or `fatal`. `info` changes no promised fact; `warning` retains trustworthy complete coverage; `error` makes a bounded requested domain unavailable/unresolved/partial while independent inspection continues; `fatal` means a required root/precondition or snapshot trust failed and dependent inspection stops. Not-requested, recognized unsupported, and intentionally encrypted-opaque states are not diagnostics alone. Anomaly severity follows consequence. Severity remains separate from availability, coverage, containment, and final process outcome, and adapters cannot alter it.
- Human decision Q6 (2026-08-19): hostile-input arithmetic rejects negative signed fields before conversion; uses checked conversion/addition/multiplication/alignment/accumulation; validates complete ranges against their immediate container and physical file before reads, slices, allocation, or iteration; and never allocates directly from an unvalidated disk value. Hard limits derive from the pinned format and trusted snapshot geometry. Every linked traversal uses visited identities and deterministic bounds; overlap/duplicate checks use only validated claims/extents. A format-limit breach is a local `error` anomaly. An operational budget stop is non-corruption `inspection.resource_limit`: retain the validated prefix, mark promised work partial, report limit/consumption/stopped boundary and known or unknown remainder, and produce a non-success outcome. No truncation, sampling, or hidden completion claim is permitted; tickets 05/06 may choose exact budgets but not semantics.
- Human decision Q7 (2026-08-19): every requested inspection facet has a coverage ledger. `complete` means every unit enumerable through trusted parents received a conclusive validation outcome, even if corruption was found; `partial` means requested units/details were blocked by an untrusted boundary, unreadable/unsupported/opaque input, resource limit, or interruption; `not-requested` remains intentional scope. Partial ledgers name the facet, examined/conclusive counts, a total only from trusted evidence, stopped boundary/reference, reason/availability, diagnostic occurrences, and known suppressed count or explicit unknown. Aggregates never extrapolate or show percentages with untrusted denominators. Any requested partial facet sets a report-level incomplete marker and prominent cross-adapter warning.
- Human decision Q8 (2026-08-19): inspection outcome precedence is `success`, `success-limited`, `findings`, `incomplete`, then `fatal`. Expected v1 limitations alone (including no-key encrypted opacity and recognized unsupported decoders) are prominent/incomplete but `success-limited` exits zero. A complete inspection with `error` corruption is `findings`; unexpected unreadability, untrusted enumeration, operational limits, or interruption is `incomplete`; required-root/configuration/snapshot-trust failure is `fatal`; all three exit nonzero. Info/warning alone remains successful. Combined results retain all axes but use the highest class. Ticket 06 assigns integers without changing zero/nonzero semantics. Interactive views show the same outcome; corruption data in a normal web response is not an HTTP transport failure.
- Human decision Q9 (2026-08-19): one canonical diagnostic occurrence is indexed once in the graph and contains ID, code, severity, anomaly flag, typed affected entities/references, evidence and canonical volume byte locators, failed boundary/containment impact, coverage-ledger references, safe message, and structured code-specific parameters. Entity/relationship/coverage objects hold occurrence IDs rather than copies. Machine consumers use codes/fields, unknown future codes render generically, and ordering is severity descending then code, canonical subject, evidence locus, and ID. All adapters show outcome/incompleteness first, unique counts, participant badges/backlinks, failure/evidence/containment/suppression detail, and text severity in addition to color/icon. Raw payload, ciphertext, keys, nonessential host paths, and unescaped disk strings are forbidden.
- Human decision Q10 (2026-08-19): the concrete containment matrix is fixed. Root profile/input/key/snapshot-trust failure is fatal. One invalid volume retains only its manifest/evidence and does not yield inferred sectors/pages; sibling volumes continue, while loss of volume-0 boot metadata disables authoritative file ownership snapshot-wide. One bad sector-bitmap page makes only its covered reservation range unknown. Tracker failure blocks inventory/ownership, not physical topology. Valid fixed-layout tracker/file entries remain independent; one bad file header/chain blocks only that file's dependent claims. One bad page envelope blocks only its body. A bad slotted container blocks its directory; after a valid directory, bad/overlapping slots block only participating record details. A bad heap OOS-reference directory yields no guessed references; valid fixed entries are independent. A bad/cyclic OOS link stops only that chain with a retained prefix/claim. Cross-source conflicts withhold only disputed derived classifications/resolution.
- Human decision Q11 (2026-08-19): the inspection core owns one versioned diagnostic catalog; adapters cannot invent or reclassify findings or remove them from the canonical projection. Every entry fixes stable meaning, default severity/anomaly flag, boundary/containment, structured parameters, required subjects/evidence, and a safe message template. Known violations receive specific namespaced codes rather than generic `corrupt`; new codes are compatible additions, while removal or semantic reuse requires a new JSON contract version. `inspection.internal_error` is a fatal non-anomaly tool bug. Catalog conformance, safe rendering, and containment/severity behavior are mandatory cross-adapter tests.

## Answer

Volmap uses **hierarchical validation boundaries** and dependency-directed salvage. A failed boundary invalidates only facts or traversals that depend on it. Independently validated parent facts, fixed-layout siblings, prior linked-structure prefixes, and unrelated entities remain usable. The sole whole-revision exception is detected snapshot mutation. No parser may clamp an invalid count, guess a child location, skip a broken link, select a winner among conflicting claims, or infer through an untrusted offset or length.

### Containment contract

| Boundary | Preserve and continue | Stop or withhold |
|---|---|---|
| Required snapshot/profile/configuration root | Input evidence and the fatal diagnostic | All dependent interpretation; this includes an unsupported/unidentifiable profile, unusable required root input, invalid explicitly supplied TDE key file, and snapshot mutation |
| One volume | Its manifest claim, bounded read evidence, and independently valid sibling volumes | Sector/page topology derived from invalid geometry; if volume 0 boot metadata is unavailable, authoritative file inventory/ownership snapshot-wide |
| One sector-bitmap page | Validated volume geometry, other bitmap pages, and physical page-envelope inspection | Reservation state only for the bitmap page's covered sector range |
| Tracker or file table | Valid physical topology, previously validated chain components, independent fixed-size entries, and other files | Tracker-dependent inventory on bootstrap failure; after a file-local failure, only that file's invalid entry/link and dependent allocation/ownership claims |
| One page envelope | Physical file location, bounded evidence, valid envelope claims, and all sibling pages | The user region and page-type decoder when identity, duplicated LSA, type, flags, read, or decryption validation fails |
| Slotted-page container or slot | Valid header facts and unrelated valid slots after directory geometry is proven | All slots if directory geometry is untrusted; otherwise only an invalid slot and record-detail decoding for overlapping participants |
| Heap OOS-reference directory | The containing slot and other page facts; independent fixed entries after directory validation | All guessed references when directory/sentinel geometry fails, or only an invalid fixed entry when the directory is valid |
| OOS chunk/link | Validated prefix, broken/closing relationship claim, affected pages/slots, and other chains | Traversal at the first invalid, missing, wrong-type, cyclic, length/index-invalid, or budget-stopped link; the chain never becomes complete |
| Redundant or ownership conflict | Every independently validated claim and endpoint | Only the disputed derived classification, semantic relationship, or resolved owner |

A bounded invalid claim remains evidence without exposing its byte contents. A fact becomes interpreted evidence only when its own range and all prerequisite boundaries validate. Invalid aggregate counters remain visible as claims beside exact independently derived counts. Once container geometry validates, fixed-position items are siblings and validate independently. Linked structures preserve a **validated prefix** but never jump past a bad link. Cycle-closing edges remain evidence, while membership contains each entity at most once. Duplicate allocation and ownership retain all claimants and resolve no winner. Overlapping slot extents retain both slot claims, block record-specific decoding of participants, and do not affect non-overlapping records.

### Arithmetic, traversal, and resource safety

Before conversion, Volmap rejects negative signed fields. All offset, length, count, alignment, page/sector address, and accumulated-size calculations use checked conversion, addition, multiplication, and alignment. A complete range must fit its immediate validated container and the physical file before reading, slicing, allocation, or iteration. No allocation is sized directly from an unvalidated disk value.

Hard structural limits come from the pinned format and trusted snapshot geometry. Every linked traversal uses a visited identity set and deterministic format/geometry bounds. Duplicate and overlap checks consume only validated claims or extents. Breaching a format limit is a local `error` anomaly. Reaching an explicit operational budget is non-corruption `inspection.resource_limit`: stop at the boundary, retain the prefix, mark the promised facet partial, report limit and consumption, name the stopped boundary, and report a derivable suppressed count or `unknown`. It produces a non-success result. Sampling, silent truncation, speculative continuation, and a complete label on budget-stopped work are forbidden.

### Diagnostic model

The core owns a versioned catalog. Each catalog definition fixes its lowercase namespaced code and meaning, default severity, anomaly flag, failed boundary and containment rule, structured parameter schema, required subjects/evidence, and safe message template. Released codes are never repurposed. Adapters cannot invent, promote, demote, or remove a finding from the canonical projection and never parse message text. A user-selected view filter may hide rows temporarily, but it cannot mutate the graph or falsify unfiltered counts.

Each occurrence has an opaque ID derived deterministically from `SnapshotId`, code, the canonical affected entity/reference set, canonical evidence or relationship locus, and an ordinal only where the same locus can contain independent violations. Revision, severity, and message text are excluded. Repeated discovery merges evidence and participants; separate corrupt entries remain separate; one cross-entity conflict remains one occurrence. Rescans intentionally create new IDs. A proven occurrence is monotonic within its snapshot: enrichment may add evidence but cannot silently remove or reinterpret it.

Severity means consequence for inspection trust:

| Severity | Meaning |
|---|---|
| `info` | Noteworthy evidence; no promised fact changes |
| `warning` | Attention is warranted, but requested facts remain trustworthy and coverage complete |
| `error` | A bounded requested domain becomes unavailable, unresolved, or partial; independent work continues |
| `fatal` | A required root/precondition or snapshot trust fails; dependent inspection stops |

Expected `not-requested`, recognized `unsupported`, and intentional `encrypted-opaque` states are not diagnostics by themselves. A readable supported format violation is an anomaly whose severity follows its consequence. `inspection.internal_error` is a fatal tool defect, never an on-disk anomaly.

The mandatory v1 catalog families are:

- root/input: `format.unsupported_profile`, `input.required_unreadable`, `input.volume_unreadable`, `snapshot.modified`;
- volume/sector: `volume.envelope.invalid`, `volume.header.invalid_magic`, `volume.header.identity_mismatch`, `volume.header.geometry_invalid`, `volume.header.strings_invalid`, `volume.chain.conflict`, `sector.bitmap.invalid`, `sector.reservation.conflict`;
- file: `file.tracker.bootstrap_invalid`, `file.tracker.item_invalid`, `file.header.invalid`, `file.accounting.mismatch`, `file.table.layout_invalid`, `file.table.reference_invalid`, `file.table.cycle`, `file.allocation.out_of_range`, `file.allocation.unreserved_sector`, `file.allocation.duplicate_sector`, `file.ownership.conflict`;
- page/TDE: `page.envelope.identity_mismatch`, `page.envelope.lsa_mismatch`, `page.envelope.type_unknown`, `page.body.invalid`, `tde.flags.invalid`, `tde.key_file.insecure_permissions`, `tde.key_error`, `tde.decrypted_invalid`;
- slot/heap: `slot.header.invalid`, `slot.entry.bounds_invalid`, `slot.entry.type_invalid`, `slot.extent.overlap`, `slot.accounting.mismatch`, `heap.oos_ref.directory_invalid`, `heap.oos_ref.entry_invalid`, `heap.oos_ref.length_mismatch`;
- OOS: `oos.chunk.header_invalid`, `oos.chain.head_invalid`, `oos.chain.target_missing`, `oos.chain.target_type_mismatch`, `oos.chain.index_mismatch`, `oos.chain.total_length_mismatch`, `oos.chain.payload_length_invalid`, `oos.chain.cycle`, `oos.chain.unterminated`;
- inspector: `inspection.resource_limit`, `inspection.interrupted`, `inspection.internal_error`.

Ticket 12 may split these families into more invariant-specific compatible codes, but cannot remove, reuse, or collapse a released meaning. Tests must prove every emitted code is cataloged, its parameters/subjects/evidence conform, its severity and containment match the definition, and every adapter renders it safely.

### Coverage and outcome

Every requested facet—such as volume topology, page envelopes, page bodies, slots, or one OOS chain—has a coverage ledger. `complete` means every unit enumerable through trusted parents received a conclusive outcome, even when corruption was found. `partial` means requested work was blocked by an untrusted boundary, unreadable/unsupported/opaque evidence, operational limit, or interruption. `not-requested` remains intentional scope.

A partial ledger names the facet, evaluated counts, a total only when independently trustworthy, stopped boundary/reference, reason code or availability, related diagnostic IDs, and a derivable suppressed count or explicit `unknown`. Aggregates never extrapolate from prefixes or subsets and never display a percentage with an untrusted denominator. Any requested partial facet sets a report-level incomplete marker.

Outcome precedence is:

1. `success`: supported requested work complete; no error/fatal.
2. `success-limited`: only expected v1 limitations, including no-key encrypted opacity or a recognized unsupported decoder; incomplete is prominent, but CLI exits zero.
3. `findings`: requested inspection completed with one or more error-level corruption findings; CLI exits nonzero.
4. `incomplete`: unexpected unreadability, untrusted enumeration, resource limit, or interruption prevented promised work; CLI exits nonzero.
5. `fatal`: required root/configuration/snapshot trust failed; CLI exits nonzero.

Combined cases retain all severity, anomaly, availability, coverage, and snapshot-validity axes but report the highest-precedence outcome. Ticket 06 assigns integer exit codes without changing zero/nonzero semantics.

### JSON and human projections

A diagnostic occurrence exists once in the graph's top-level index. It contains occurrence ID, code, severity, anomaly flag, affected typed entities/references, evidence references and canonical volume offset/length locators, failed boundary and containment impact, related coverage ledgers, structured parameters, and a safe human message. Entities, relationships, and coverage ledgers backlink by occurrence ID rather than copying objects. No diagnostic contains raw payload, ciphertext, keys, nonessential host paths, or unescaped on-disk strings.

Canonical order is severity descending, then code, canonical subject, evidence locus, and occurrence ID. JSON clients use codes and structured fields; unknown future codes remain generically renderable. CLI, TUI, HTML, and web show outcome/incompleteness first, count unique occurrences by severity/code, badge every affected entity or relationship, and expose a detail view with failure, evidence location, containment effect, suppressed/unknown scope, and navigation to all participants. Severity always has a text label in addition to color or icon. Normal web responses carrying corrupt inspection results remain successful HTTP transports; the inspection outcome stays in the model.
