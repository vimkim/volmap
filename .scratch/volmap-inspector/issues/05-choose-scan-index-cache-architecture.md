Type: grilling
Status: resolved
Blocked by: 01, 02, 04

# Choose the scan, index, and cache architecture

## Question

How should the selected implementation platform turn immutable volume files into the canonical inspection model without loading an entire large database into memory? Decide module seams for discovery, bounded byte access, format profiles, allocation and ownership indexing, fast summaries, lazy page decoding, OOS chain traversal, caching or temporary spill files, snapshot fingerprints, cancellation, and concurrent CLI/TUI/web requests. Quantify asymptotic behavior and define which work happens at startup versus selection time. Consult `codebase-design` so format complexity remains behind deep interfaces rather than leaking into every presentation adapter.

## Comments

### Design exploration (unresolved)

- The current representative snapshot has a 64 MiB primary volume and a 128 MiB extension, totaling 12,288 physical pages. The architecture must remain bounded for substantially larger snapshots; this measurement does not establish final resource budgets.
- Three interface shapes were compared: a minimal revision-aware `open/query/enrich` module, a compile-time facet engine over a virtual columnar graph, and a common-case `open/summary/select` module over packed indexes.
- The current recommendation combines a small revision-aware inspection interface with a virtual packed graph and keeps facet producers, format decoders, I/O, indexing, spill, and caches behind the seam. Presentation adapters would receive immutable revision-pinned graph views and could never decode or derive facts.
- At this exploration point, the external interface shape, index lifecycle, initial publication, and enrichment/concurrency semantics were still pending; Q1-Q4 below record their later resolution.
- Human decision Q1 (2026-08-19): all adapters use one revision-aware inspection module. `Inspection::open` creates the session, `Inspection::view` returns an immutable revision-pinned graph view, and `Inspection::enrich` performs explicit targeted deep inspection. Canonical graph queries live on the view; summary and selection helpers may wrap them. Facet producers, decoders, storage, and presentation-specific operations do not enter the external interface.
- Human decision Q2 (2026-08-19): the canonical index uses a segmented memory-first store that automatically spills interface-safe facts to private session-only storage under a hard resident-memory budget. Version one never reuses an index across processes; every rescan receives a new `SnapshotId`. Raw pages, ciphertext, plaintext application regions, nonces, keys, and application values are never spilled or cached.
- Human decision Q3 (2026-08-19): revision zero is published atomically only after the complete unsampled fast scan terminates. Progress may be reported separately, but adapters never query a graph while its initial topology is being mutated. A bounded stop after the required root exists may publish an explicitly partial revision zero under the resolved coverage/outcome semantics; it is not mislabeled complete.
- Human decision Q4 (2026-08-19): deep work decodes outside graph locks, coalesces concurrent work for the same target, and commits one request's validated facts atomically as a new immutable revision. Queries pin exactly one revision and never observe a partial commit. Independent targets may execute concurrently; revision publication is serialized and deterministic.
- Human decision Q5 (2026-08-19): the graph store materializes volumes, sectors, files, sparse findings, and deep details, but represents page topology virtually from validated geometry and packed sector/allocation masks. Compact fast facts exist only for system and allocated pages whose envelopes are inspected; canonical page entities are synthesized when queried. Volmap does not embed a general database or allocate one heap object per physical page.
- Human decision Q6 (2026-08-19): fast inspection runs in the prerequisite order discovery, volume geometry, sector bitmaps, authoritative tracker/files, allocation claims, page envelopes, and reconciliation. Ordinary envelope inspection reads only the 32-byte prefix and 8-byte watermark in physical page order through bounded workers. Full bodies are read only for required metadata structures or explicit deep inspection.
- Human decision Q7 (2026-08-19): one operational policy budgets resident bytes, spill bytes, workers, traversal steps, and decoded bytes. Work that cannot be admitted within its budget does not begin; exhaustion retains validated facts and produces `inspection.resource_limit`, partial coverage, and a non-success outcome. Numeric defaults require representative measurements after this architecture closes and are not inferred from the small current fixture.
- Human decision Q8 (2026-08-19): Volmap holds read-only volume handles for the session and fingerprints `_vinf` identity, device/inode, size, nanosecond timestamps, validated volume identity/creation fields, and fast-scan page envelopes. It checks all volume stamps before and after fast inspection and each enrichment job, and compares selected pages with their original envelopes. Any mismatch terminally invalidates the snapshot; full-volume hashing is not required.
- Human decision Q9 (2026-08-19): cancellation before a safe snapshot root exists is an `OpenFailure`. After the root exists, an interrupted operation atomically publishes only its validated facts with `inspection.interrupted` and truthful partial coverage. A canceled page-body decode contributes no partial body interpretation; a canceled OOS traversal may retain its validated chain prefix. Canceling a query never mutates graph state.
- Human decision Q10 (2026-08-19): canonical facts and committed enrichment are graph state and cannot be evicted. A byte-weighted bounded LRU may contain only safe index blocks and sanitized structural projections. Raw pages, ciphertext, decrypted regions, nonces, keys, and application values are never cached.
- Human decision Q11 (2026-08-19): `inspection` is the only external seam. Internal deep modules own `source` (handles, reads, fingerprints), `format` (pinned decoders), `fast_scan`, `ownership`, `graph_store`, `deep_inspection`, and `tde`. Raw decoders become crate-private once the seam exists. Production-file and hostile-fixture adapters implement an internal `VolumeSource` interface without exposing dependency injection to presentation adapters.
- Human decision Q12 (2026-08-19): `GraphView` accepts a closed typed canonical query vocabulary for overview, entity enumeration/detail, relationships, diagnostics, evidence, and coverage. Revision-bound opaque pagination cursors bind snapshot, revision, query, and canonical order. The interface exposes neither SQL nor callbacks, generic graph traversal, storage concepts, or presentation-specific queries.
- Human decision Q13 (2026-08-19): revision zero and all append-only enrichment records remain available for the session. A revision is a high-water mark over monotonic records rather than a graph copy, so any session revision can be reopened. Spill limits bound history; version one does not compact in a way that invalidates a published revision.
- Human decision Q14 (2026-08-19): file scans emit compressed sector/page allocation masks. The store externally sorts claims when the resident budget requires it, then deterministically reduces them by physical identity, preserves all claimants, resolves ownership only for exactly one validated claim, and accumulates canonical sector/file summaries during the same reduction. Adapters never recompute ownership or summaries.
- Human decision Q15 (2026-08-19): one enrichment transaction targets one page or one OOS chain and publishes at most one revision. A request already complete at the requested scope is an idempotent no-op. Work stopped by interruption or an operational limit may explicitly resume from its validated boundary; conclusive corrupt, opaque, or unsupported results are not repeatedly decoded.
- Human decision Q16 (2026-08-19): supported input failures and limitations—including corruption, unreadability, unsupported detail, opacity, interruption, and resource exhaustion—live in canonical availability, diagnostics, coverage, and outcomes. Operation errors are limited to invalid selectors/cursors, inability to establish the required root/store, terminal snapshot invalidation, and inspector defects. Adapters cannot reclassify either category.
- Human decision Q17 (2026-08-19): synchronous `open` accepts a safe ephemeral progress observer. It reports the current scan phase, completed units, and only an independently trusted optional total; it exposes no paths or bytes and never reports a percentage with an untrusted denominator. Progress is not inspection-graph evidence and does not publish a mutable revision.
- Human decision Q18 (2026-08-19): version one uses sealed enum dispatch for the single pinned format profile and a compile-time internal decoder registry. It has no public profile/decoder plugin interface. A new seam is justified only when a second authoritative profile exists; dynamic plugins are out of scope.
- Human decision Q19 (2026-08-19): spill uses a private mode-`0700` session directory and mode-`0600` segment files, rejects unsafe locations, unlinks open segments immediately where supported, and cleans up on normal shutdown. It stores only interface-safe facts and never falls back to unbounded RAM. Failure before the required store root exists is an open/root failure; later capacity exhaustion is `inspection.resource_limit` with retained facts and partial coverage.

## Answer

Volmap uses one deep, revision-aware **inspection module** between immutable volume sources and every CLI, JSON, TUI, HTML, and web adapter. It performs the complete fast scan, owns the canonical graph and all derivation, and exposes immutable revision views plus explicit targeted enrichment. Format decoders, file handles, scan ordering, caches, spill files, worker pools, and TDE material never cross this seam.

An illustrative interface is:

```rust
Inspection::open(OpenRequest, ResourcePolicy, CancelToken, ProgressObserver)
    -> Result<Inspection, OpenFailure>
Inspection::view(RevisionSelector) -> Result<GraphView, OperationError>
Inspection::enrich(DeepRequest, CancelToken)
    -> Result<RevisionReceipt, OperationError>
GraphView::query(GraphQuery) -> Result<QueryPage, QueryError>
```

`GraphQuery` is a closed typed vocabulary for overview, entity enumeration/detail, relationships, diagnostics, evidence, and coverage. It is canonical-model-oriented rather than presentation-oriented. Opaque cursors bind the snapshot, revision, complete query, and canonical ordering. Convenience summary and selection helpers may compile to these queries. There is no SQL, arbitrary callback, generic graph traversal, public decoder interface, or dynamic plugin system.

### Module seams

- `inspection` owns lifecycle, the external interface, initial publication, revision coordination, outcomes, and snapshot invalidation.
- `source` discovers and holds the immutable input set, performs positional reads, and owns manifest/page fingerprint verification. Its private `VolumeSource` interface has production file-handle and hostile in-memory fixture adapters.
- `format` owns the pinned offsets, endian rules, layout validation, and safe decoders. Version one uses sealed enum dispatch for the single pinned profile and a compile-time internal decoder registry; current raw decoder interfaces become crate-private once `inspection` exists.
- `fast_scan` executes the prerequisite graph and coverage ledgers.
- `ownership` decodes tracker/file tables, preserves allocation claims, reduces conflicts, resolves unique owners, and derives allocation summaries.
- `graph_store` owns canonical entities/facts, relationships, evidence, diagnostics, coverage, indexes, revisions, spill, and safe hot caches.
- `deep_inspection` validates selected page bodies, slotted structures, record metadata, and bounded OOS chains.
- `tde` owns key-file validation, secret lifetime, decryption, and zeroization. Decrypted buffers yield only permitted structural facts and never enter graph storage or caches.

The bounded byte-access and source seams accept dependencies rather than opening paths from decoders. Production reads use stable read-only handles and positional offsets; tests use scripted short-read, truncation, mutation, and fault adapters. Decoders receive only a validated bounded container or prerequisite capability, never an unrestricted volume reader.

### Initial fast inspection

`open` performs this dependency-ordered pass:

1. Resolve the supplied database/`_vinf` input under the later CLI policy, open every accepted volume read-only, capture the pre-scan manifest, and establish the pinned profile and required snapshot root.
2. Decode and validate volume envelopes, headers, geometry, and sector bitmaps.
3. Bootstrap volume-0 metadata, the authoritative file tracker, file headers, and extensible allocation tables. If an explicit TDE key file was supplied, validate it during `open`, obtain the permanent data key through the resolved boot metadata, and zeroize master and temporary key buffers; no secret enters the graph store.
4. Emit compressed per-file sector/page allocation masks; externally sort them when the resident budget requires it; reduce them deterministically by physical identity. Preserve every claim, resolve ownership only for exactly one validated claimant, and accumulate sector/file summaries during the reduction.
5. Inspect the plaintext envelope of every system or allocated page in physical volume/page order. Ordinary pages require only the 32-byte prefix and 8-byte watermark; full bodies are read only for required metadata structures. Bounded workers may read independently, but their segments merge in canonical order.
6. Reconcile reservation, allocation, ownership, page type/TDE state, evidence, coverage, diagnostics, and exact summaries; recheck all input fingerprints; then atomically publish revision zero.

The fast scan is complete and unsampled when every unit enumerable through trusted parents has a conclusive outcome. A corruption boundary, unreadable input, resource stop, or interruption after the required root exists may instead publish one explicitly partial revision zero with retained validated facts and the already-defined outcome. Adapters never query an actively mutating initial graph. A safe progress observer may report phase, completed units, and an independently trusted optional total while `open` runs, but progress is ephemeral, contains no paths or bytes, and is not graph evidence.

### Virtual graph and bounded store

The canonical graph is semantic; it is not represented as one heap object per entity. Volumes, sectors, files, sparse findings, and requested deep details are materialized records. All physical pages still exist canonically, but untouched page entities are synthesized from validated volume geometry plus packed system, reservation, and allocation masks. Fixed-width fast facts are stored only for system and allocated pages whose envelopes were inspected. Allocation claims remain compressed by sector/page masks, and conflicts use sparse side records.

The store writes immutable sorted base segments for revision zero and append-only enrichment segments afterward. A revision is a high-water mark over monotonic records, not a copy of the graph, so every published session revision remains reopenable. Deep facts are canonical state and cannot be evicted. Version one performs no compaction that would invalidate a revision.

Storage begins in bounded memory and flushes segments automatically to private session-only spill. The session directory is mode `0700`; files are mode `0600`, reject unsafe locations, and are unlinked while open where the platform supports it. Normal shutdown removes remaining artifacts. Spill contains only interface-safe canonical facts—never raw pages, ciphertext, decrypted application regions, nonces, keys, or application values. Volmap never falls back to unbounded RAM and never reuses an index across processes; each rescan creates a new `SnapshotId`.

A byte-weighted LRU may retain only safe index blocks and already-sanitized structural projections. Raw/decrypted page buffers are bounded per worker, are not cached, and are zeroized after permitted facts have been extracted. Committed enrichment is stored or spilled rather than evicted.

### Targeted enrichment and revisions

One `DeepRequest` names exactly one page or one OOS value chain. Page enrichment rechecks the snapshot, performs one bounded page read, optionally decrypts it, validates the complete prerequisite structure, and publishes only safe page/slot/detail facts. Heap OOS references remain visible but do not auto-traverse. OOS enrichment follows validated chunk links with visited identities plus explicit step and decoded-byte budgets, retains a validated prefix on a bounded stop, and never retains payload fragments.

Enrichment parses outside graph locks and publishes at most one atomic revision. A completed target at the requested scope is an idempotent no-op. Work stopped by interruption or an operational budget may resume explicitly from its validated boundary; conclusive corruption, opaque encryption, or recognized unsupported detail is not repeatedly decoded. Equal in-flight targets coalesce when the active request satisfies the waiting request's budget; independent targets run concurrently. Revision publication alone is serialized, with canonical merge and diagnostic ordering. Queries pin one immutable revision and cannot observe a partial commit.

### Fingerprints, cancellation, and failures

The input manifest records discovery-file identity, held-handle device/inode, size, nanosecond modification/change timestamps, and validated volume identity/creation facts. Fast facts retain each inspected page's identity, LSA, type, and TDE envelope state. All volume stamps are checked before and after fast inspection and every enrichment job; a selected page must also match its original envelope. Full-volume hashing is not required. Any mismatch atomically raises `snapshot.modified`, cancels outstanding enrichment, terminally invalidates the snapshot, and makes retained facts diagnostic-only.

Cancellation is observed only at I/O batches and validated structure/link boundaries, never midway through publication. Before a required root/store exists it yields `OpenFailure`. Afterward, startup or enrichment publishes only its validated facts with `inspection.interrupted` and truthful partial coverage. A canceled single-page body produces no partial body interpretation; a canceled OOS traversal may publish its validated prefix. Query cancellation returns without graph mutation. One waiter's cancellation does not stop shared in-flight work still required by another waiter.

Corruption, unreadability, unsupported detail, encrypted opacity, interruption, and resource exhaustion are supported inspection results expressed through canonical availability, diagnostics, coverage, and outcome. Operation errors are limited to invalid selectors/cursors, inability to establish the required root/store, terminal snapshot invalidation, and inspector defects. A store-creation failure is a root/open failure; later memory or spill exhaustion is `inspection.resource_limit`, retains validated facts, and produces partial coverage and a non-success outcome.

### Resource and asymptotic contract

One `ResourcePolicy` limits resident bytes, spill bytes, worker count, linked-traversal steps, and decoded bytes. Work that cannot be admitted does not start. Numerical defaults require representative measurements and are intentionally delegated to a newly surfaced research ticket rather than inferred from the small 192 MiB/12,288-page development snapshot.

Let `V` be volumes, `S` sectors, `F` files, `C` validated compressed allocation-claim records, `A` system/allocated pages requiring envelope inspection, `D` committed deep records, `K` chunks in one selected OOS prefix, `R` returned query rows, `B` the resident-byte budget, and `W` admitted workers.

- Fast inspection performs `Theta(V + S + F + A)` validation/reduction work plus a bounded external sort of claims, worst-case `O(C log C)` comparisons and external merge I/O determined by `B`. Logical ordinary-envelope input is 40 bytes per page, subject to filesystem block amplification.
- Resident memory is `O(B + V + W * 16 KiB)` and therefore independent of total page count once the explicit handle/worker terms are accounted for. Spill is `O(V + S + F + C + A + D)` compact records; there is no automatic `O(P)` heap graph for all geometrically implied pages.
- Precomputed overview and aggregate summaries are `O(1)`. A physical page lookup is `O(1)` through volume/sector rank and packed masks; indexed entity lookup is at most logarithmic; ordered enumeration is `O(log N + R)` or `O(R)` after a cursor seek.
- One selected page costs one 16 KiB read plus decoder work. A slotted page is linear in validated slots apart from deterministic extent-overlap ordering. An OOS chain costs `Theta(K)` bounded reads/time and `O(K)` visited identities, within its operational policy.
- Concurrency cannot change graph meaning, evidence identity, diagnostic ordering, summaries, or query order; it changes only wall-clock scheduling within the same admitted budgets.
