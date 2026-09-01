Type: task
Status: resolved
Blocked by: 01, 03

# Build normalized page-file-class associations

## Question

Can the validated `VPID -> VFID -> descriptor class_oid -> class-name resolution` relation be made a snapshot-scoped production inspection fact without growing the compact per-page fact or mislabeling pages when evidence is incomplete?

Add the shared typed file/class association model and normalized storage: reuse the validated page allocation map, retain descriptor associations by VFID, collect each distinct non-null class OID only after complete trustworthy inventory, and resolve each OID once into a shared cache. Join the association when producing `PageView`; do not copy a class-name `String` into every packed physical-page fact. Keep allocation, file identity/role, class OID, name resolution, and non-applicability distinct. Fail closed on incomplete inventory, absent headers, conflicting owners, corrupt/encrypted evidence, null/system associations, and dangling OIDs. Heap, heap-reuse, multipage heap overflow, B-tree, B-tree-overflow-key, extensible hash, and hash directory pages must obtain associations from their allocating file; catalog/global/internal/unallocated/reserved-unallocated pages must not become user-table-owned; `FILE_OOS` must be typed as deferred/non-applicable or unresolved without inference. Add model/inventory/query tests for every state and preserve the physical page-fact size/memory contract. Format, run the focused tests and relevant resource benchmark, and commit the ticket's production change on `feat/page-table-attribution`.

## Comments

## Answer

Implemented the normalized association at the shared inspection/projection seam. The complete validated file inventory is now the sole boundary that can promote descriptor class OIDs and populate the snapshot-scoped class-name cache. Selective file enrichment may retain a typed `VFID` and `FileType`, but reports the class association as unresolved until the inventory is complete and never publishes a partial OID or name.

`ClassAssociation` distinguishes unresolved evidence, typed non-applicability, and a retained class OID with resolved or typed-unresolved name evidence. `PageFileAssociation` continues to distinguish allocation, reservation, mixed claims, and no relationship. The normalized `file_allocations`, `tracked_files`, and per-distinct-OID `class_names` maps are joined only when producing the shared `PageView`; the packed physical-page fact remains exactly 16 bytes and stores no class-name string. Test fixtures use the production `FileType` enum, with its raw on-disk discriminator centralized in `FileType::ordinal()`.

The public graph/projection tests cover incomplete selective enrichment, absent tracker-referenced headers, conflicting owners, all seven scoped file families sharing one `Arc<str>` resolution, null/internal/no-single-class/deferred-OOS states, allocated/reserved/mixed/unowned pages, and the serialized unresolved/not-applicable distinctions. The read-only oracle still resolves volume 1 page 1000 through file `1:640` and class OID `0:209:2` to `dba.poc_table`.

Verification passed:

- `cargo fmt --all -- --check`
- `cargo clippy --locked --all-features --all-targets -- -D warnings`
- `cargo test --locked --test class_name_resolution --test file_table --test inspection_scan`
- `cargo test --locked --lib inspection::tests::packed_page_fact_is_exact_and_round_trips_canonical_fields -- --exact`
- `cargo test --locked --release --test resource_benchmark -- --ignored --nocapture`
- `cargo test --locked --all-features --all-targets --quiet`
- independent Standards and Spec reviews against prerequisite commit `79dd654` (zero findings after fixes)
