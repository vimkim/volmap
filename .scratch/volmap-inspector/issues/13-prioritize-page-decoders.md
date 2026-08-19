Type: grilling
Status: resolved
Blocked by: 04

# Prioritize version-one page-type decoders

## Question

Beyond the generic physical page envelope, volume/file allocation metadata, common slotted-page structure, and required OOS views, which pinned physical page types and recognized record structures must version one decode semantically? Use the complete format inventory to classify each page type as required decoder, generic structural view only, or explicitly opaque. Balance diagnostic value, source complexity, corrupt-input risk, UI usefulness, and acceptance-fixture cost; define how unsupported-but-valid types appear without being mislabeled corrupt.

## Comments

### Standing human disposition

On 2026-08-19 the user directed every remaining ticket to accept the source-backed recommended option and continue without further HITL. This ticket therefore records each recommendation as accepted once its factual prerequisites are established; the instruction does not relax source, fixture, hostile-input, or completion audits.

### Pinned inventory and fixture facts

- The product profile remains commit `e1e651debf6cc100172bde96603b17424f9c135a`, even though the local `feat/oos` worktree has advanced. The pinned enum has exactly fifteen ordinals: `UNKNOWN`, `FTAB`, `HEAP`, `VOLHEADER`, `VOLBITMAP`, `QRESULT`, `EHASH`, `OVERFLOW`, `OOS`, `AREA`, `CATALOG`, `BTREE`, unused `LOG`, `DROPPED_FILES`, and `VACUUM_DATA` ([pinned format report](../research/pinned-disk-format.md), `src/storage/storage_common.h:148-167` at the pinned commit).
- The common slotted-page structure applies to exactly `HEAP`, `OOS`, `BTREE`, `EHASH`, and `CATALOG` (`src/storage/slotted_page.c:1113-1133` at the pinned commit). A valid generic slot map for one of these types does not imply semantic support for its records.
- Current Volmap tests synthesize only the page envelope, volume header, and sector bitmap. There are no immutable FTAB, slotted, heap, OOS, B-tree, E-hash, catalog, overflow, vacuum, dropped-file, query-result, or area fixtures. The two hashes recorded in `provenance.toml` no longer match the mutable external files, and the original bytes were not retained, so they cannot serve as acceptance goldens.
- Exact-commit OOS unit/SQL tests provide reproducible generation recipes for header/data pages, single and multi-chunk values, exact chunk boundaries, heap OOS references, and a valid non-OOS `REC_BIGONE`. Those transient databases must be captured, hash-labeled, and annotated before becoming Volmap acceptance fixtures. Recovered artifacts remain behavioral corroboration only and are excluded from tests.
- Fixture economics are low-to-medium for allocation, generic slots, OOS, ordinary heap structure, and overflow chains; medium for B-tree node headers; and high for semantic B-tree records, catalog records, E-hash, query-result, area, dropped-files, and vacuum-data bodies. Unsupported-but-recognized bodies therefore need an explicit non-corruption representation rather than speculative parsers.

### Accepted recommendations under the standing disposition

The source inventory establishes the prerequisites for the following recommendations. Under the user's 2026-08-19 blanket `accept all` direction, all eight are accepted without another HITL round:

1. **Three-level support promise.** Every recognized physical page type has one versioned page-detail promise: `semantic`, `structural-only`, or `opaque`. This promise is independent of whether a particular page is readable, encrypted, valid, requested, or completely inspected.
2. **Core semantic family.** Version one semantically decodes the already-required `PAGE_VOLHEADER`, `PAGE_VOLBITMAP`, `PAGE_FTAB`, and `PAGE_OOS` families plus bounded `PAGE_HEAP`, `PAGE_OVERFLOW`, `PAGE_CATALOG`, `PAGE_BTREE`, `PAGE_DROPPED_FILES`, and `PAGE_VACUUM_DATA` structural metadata. “Semantic” never authorizes application-value decoding.
3. **Heap and overflow boundary.** Heap decoding covers heap header/chain records, the MVCC/object envelope, representation and offset-width facts, typed slot states, OOS markers/references, and relocation/big-record forwarding. Overflow decoding covers head/rest role, declared length, next-page links, payload extents, and completeness. Heap attributes and overflow payload bytes remain opaque.
4. **Minimal catalog prerequisite.** Catalog decoding is limited to page/continuation headers, representation directories, class/representation identifiers, fixed/variable counts and lengths, attribute identifiers/locations/storage types, and structural B-tree statistics needed to bound heap layouts. It skips default values and all root-class variable content such as names, SQL, comments, methods, properties, enum labels, partition expressions/values, and JSON schemas.
5. **B-tree structural semantics.** B-tree decoding covers root/node/OID-overflow headers, node role and level, key counts, sibling/child/overflow links, fixed record flags, and bounded key/object extents. It never deserializes or compares key values. Any structure whose boundary cannot be proven without interpreting a key remains opaque rather than guessed.
6. **Generic and opaque families.** `PAGE_EHASH` is `structural-only`: a bucket may receive the common slotted-page view only after file/page role is proven, while directory/key semantics remain unsupported. `PAGE_UNKNOWN`, `PAGE_QRESULT`, `PAGE_AREA`, and the unused database-volume `PAGE_LOG` have opaque bodies in version one. Query tuples, E-hash keys, and consumer-defined area bytes are never parsed.
7. **Unsupported is not corruption.** A recognized page with an intentionally unsupported detail promise reports that page type, allocation/ownership, safe envelope facts, detail availability `unsupported`, and truthful partial coverage. It produces `success-limited` when no independent anomaly exists. An unknown ordinal, invalid envelope, impossible file/page role, or violated invariant remains a diagnostic; `PAGE_UNKNOWN` itself is a recognized deallocated/uninitialized value rather than the `page.envelope.type_unknown` condition.
8. **Acceptance-fixture gate.** No semantic decoder is complete until tests include a source-derived valid fixture, boundary/malformed mutations, output non-disclosure assertions, and cross-reference/containment cases. Fixtures must be generated from pinned commit `e1e651de`, use synthetic content, retain commands/configuration/page annotations/hashes, and be copied into an immutable corpus. Mutable external volumes and recovered artifacts cannot be acceptance goldens.

## Answer

Version one uses a deliberately asymmetric decoder inventory. The page envelope, allocation topology, and common slotted-page geometry are universal foundations, but a recognized physical type does not automatically authorize subsystem-record parsing. Each type is assigned one stable **page detail support** level:

| Value | Pinned page type | Version-one support | Exact boundary |
|---:|---|---|---|
| 0 | `PAGE_UNKNOWN` | Opaque | Report the recognized deallocated/uninitialized type and topology relationship; never interpret its body. Allocation to a live file is a separate anomaly. |
| 1 | `PAGE_FTAB` | Semantic | Decode file headers, tracker items, extensible allocation tables, counters, descriptors, and links under the already-resolved allocation contract. |
| 2 | `PAGE_HEAP` | Semantic | Decode common slots, heap header/chain metadata, safe record envelopes, typed forwarding, and OOS references; never decode attribute values. |
| 3 | `PAGE_VOLHEADER` | Semantic | Decode the pinned volume header and variable string boundaries, while redacting host paths and remarks from projections. |
| 4 | `PAGE_VOLBITMAP` | Semantic | Decode bounded sector-reservation bits and reconcile them with volume geometry and file claims. |
| 5 | `PAGE_QRESULT` | Opaque | Recognize a temporary query-result page but do not decode its page header, tuples, overflow tuples, or values. |
| 6 | `PAGE_EHASH` | Structural-only | When file/page role proves a slotted bucket, expose only the common slot map. Directory records, associated OIDs, key types, and key bytes are not semantically decoded. |
| 7 | `PAGE_OVERFLOW` | Semantic | Decode first/rest headers, total length, next-page topology, payload extents, and chain completeness; never expose reconstructed payload. |
| 8 | `PAGE_OOS` | Semantic | Decode the required OOS header/data roles, safe statistics, chunk headers, chain topology, lengths, and heap inline references; never expose payload fragments. |
| 9 | `PAGE_AREA` | Opaque | Recognize the temporary/consumer-defined area page and expose no body interpretation. |
| 10 | `PAGE_CATALOG` | Semantic, minimal | Decode only the representation metadata required to prove heap layout boundaries; skip catalog defaults and root-class variable content. |
| 11 | `PAGE_BTREE` | Semantic, structural | Decode node/root/overflow headers, roles, counts, record flags/extents, and structural links without deserializing keys or comparing ordering. |
| 12 | `PAGE_LOG` | Opaque | Recognize the enum as unused in database volumes. Active/archive log files use a different envelope and remain outside version-one inputs. |
| 13 | `PAGE_DROPPED_FILES` | Semantic | Decode the bounded next-page link and `(VFID, MVCCID)` entry array; validate count, capacity, file role, and cycles. |
| 14 | `PAGE_VACUUM_DATA` | Semantic | Decode queue links/indexes and bounded block/LSA/MVCC entry metadata; validate flags, ordering, capacity, role, and cycles. |

TDE availability precedes this table. Without a valid explicit key, an AES/ARIA page retains only its plaintext envelope and is `encrypted-opaque`; Volmap never runs even a required semantic or structural decoder on ciphertext. A valid page of a recognized `structural-only` or `opaque` family is likewise an expected limitation, not corruption.

### Heap records and overflow

The common slot directory recognizes the pinned four-bit record vocabulary: `REC_UNKNOWN`, `REC_ASSIGN_ADDRESS`, `REC_HOME`, `REC_NEWHOME`, `REC_RELOCATION`, `REC_BIGONE`, `REC_MARKDELETED`, `REC_DELETED_WILL_REUSE`, and reserved values 8 through 15. Volmap always reports the numeric and recognized record kind after slot geometry is trusted. A reserved or unsupported kind is not called corrupt solely for being reserved; an impossible use in a proven subsystem role may still be an anomaly.

For heap pages, slot zero is interpreted according to proven file/page role as either `HEAP_HDR_STATS` or `HEAP_CHAIN`. Safe fields include class/file identities, page-chain links, OOS file identity, unfill policy, flags, and estimate labels; estimates never become allocation truth. A `REC_RELOCATION` record yields one typed target OID expected to be `REC_NEWHOME` in the same heap file. A `REC_BIGONE` yields the typed head of a `PAGE_OVERFLOW` chain. Self-links, cycles, cross-file links, wrong page/record types, and depth/byte-budget stops retain a validated prefix and canonical diagnostic/coverage evidence.

For `REC_HOME` and `REC_NEWHOME`, Volmap may decode only the MVCC/object envelope: representation ID, CHN, presence and bounded values of optional MVCC identifiers/previous-version locator, offset-width flags, HAS_OOS, and structurally proven fixed/variable ranges. Attribute counts and locations come only from a validated minimal catalog representation for the same class and representation ID. Missing catalog metadata makes attribute layout `unresolved`; it does not trigger guessed sentinel scans. Bound/null/OOS markers and 16-byte OOS inline stubs may be exposed as structural facts, but every ordinary attribute extent remains an opaque application-payload range.

Overflow traversal distinguishes a head page—whose fixed header declares total logical length—from continuation pages, whose fixed header has only the next VPID. It validates type, owning overflow file, positive and bounded total, per-page payload extent, exact accumulated length, terminal link, and cycles. The graph stores extents and relationships, not reconstructed record bytes.

### Catalog and B-tree limits

The catalog decoder exists only to prove physical heap layouts. It follows validated catalog page and record continuations and decodes representation-directory items, class info, disk-representation counts/fixed length, and attribute ID/location/storage type/value-length boundaries. It may retain safe identifiers and structural statistics. It skips default-value bytes and does not decode the ordinary root-class heap record's variable fields. If the minimal metadata is unavailable, the affected heap record remains a valid slot with an unresolved semantic layout.

The B-tree decoder stops at structural topology. From validated slot-zero headers it identifies root, non-leaf, leaf, and OID-overflow roles; reports level, counts, sibling/child/overflow relationships, safe root flags and file identities; and bounds each record's preamble, key extent, object extent, and overflow references where the layout is provable. It does not invoke type-specific key readers, reconstruct prefix-compressed keys, emit keys, compare ordering, or claim semantic index correctness. A record variant requiring unbounded or value-aware parsing becomes an opaque record detail while independently valid node facts remain usable.

### Structural-only and opaque reporting

`PAGE_EHASH` reuses one physical type for role-dependent raw directory and slotted bucket layouts. Without a semantic E-hash decoder, Volmap may show the common slot structure only when validated file metadata independently proves a bucket; otherwise its body is opaque. It never interprets directory depth, bucket keys, associated objects, or hash ordering.

Opaque pages still have canonical page entities. Their projections include snapshot-scoped identity, volume/file allocation and ownership claims, physical type, TDE state, safe envelope evidence, and `detail_support: opaque`. They contain no body-derived fact or raw byte. A request for unavailable semantics records `availability: unsupported`, partial coverage, and the accepted `success-limited` outcome unless another independent finding dominates. `structural-only` pages behave the same for semantic detail beyond their advertised structure.

### Acceptance matrix

Implementation must create an immutable, source-derived corpus from the pinned engine rather than depending on mutable external database files. At minimum it contains:

- a minimal stopped database with annotated volume, bitmap, tracker/FTAB, heap header/data, catalog, B-tree root/non-leaf/leaf, E-hash bucket, dropped-file, vacuum-data, and recognized unknown pages;
- OOS header/data pages with single-chunk, exact-boundary, multi-chunk, multiple-values-per-page, and matching heap inline-reference cases;
- a valid non-OOS `REC_BIGONE` and its complete overflow chain, plus relocation/forwarding cases where the pinned engine can generate them reproducibly;
- synthetic recognition cases for opaque `QRESULT`, `AREA`, and unused `LOG` without treating their bodies as semantic goldens;
- AES and ARIA generated cases under the separate accepted TDE contract;
- targeted mutations for every decoded count, offset, width, type, link, role, overlap, cycle, accumulated length, and resource boundary; and
- adapter assertions that no application value, key, default, tuple, payload fragment, ciphertext, nonce, key material, or source path reaches the graph, JSON, terminal, TUI, web API, HTML, diagnostic, or log.

Each captured fixture records the exact engine commit, build/profile configuration, generation command, clean-shutdown state, volume/page/slot annotations, and cryptographic hashes. Generated source and fixtures are authoritative under the provenance policy; recovered observations may corroborate normalized behavior but never define layout or acceptance output.
