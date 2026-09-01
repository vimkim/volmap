Type: task
Status: open
Blocked by: 01, 03

# Build normalized page-file-class associations

## Question

Can the validated `VPID -> VFID -> descriptor class_oid -> class-name resolution` relation be made a snapshot-scoped production inspection fact without growing the compact per-page fact or mislabeling pages when evidence is incomplete?

Add the shared typed file/class association model and normalized storage: reuse the validated page allocation map, retain descriptor associations by VFID, collect each distinct non-null class OID only after complete trustworthy inventory, and resolve each OID once into a shared cache. Join the association when producing `PageView`; do not copy a class-name `String` into every packed physical-page fact. Keep allocation, file identity/role, class OID, name resolution, and non-applicability distinct. Fail closed on incomplete inventory, absent headers, conflicting owners, corrupt/encrypted evidence, null/system associations, and dangling OIDs. Heap, heap-reuse, multipage heap overflow, B-tree, B-tree-overflow-key, extensible hash, and hash directory pages must obtain associations from their allocating file; catalog/global/internal/unallocated/reserved-unallocated pages must not become user-table-owned; `FILE_OOS` must be typed as deferred/non-applicable or unresolved without inference. Add model/inventory/query tests for every state and preserve the physical page-fact size/memory contract. Format, run the focused tests and relevant resource benchmark, and commit the ticket's production change on `feat/page-table-attribution`.

## Comments
