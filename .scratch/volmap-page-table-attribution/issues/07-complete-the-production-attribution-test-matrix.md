Type: task
Status: open
Blocked by: 05

# Complete the production attribution test matrix

## Question

Does the integrated production path have enough source-derived and hostile-input coverage to prove all scoped file roles, record shapes, codesets, and fail-closed boundaries without relying on a mutable external database as a committed golden?

Audit the tests landed with the decoder, resolver, model, and projection tickets; add only the missing fixtures and integration cases. Cover heap/heap-reuse, multipage heap overflow, B-tree, B-tree-overflow-key, extensible hash, and hash directory descriptors; catalog/global/internal/unallocated/reserved-unallocated and deferred OOS non-attribution; complete versus incomplete inventory; distinct-OID single resolution; compact page-fact size; home/new-home, bounded relocation, and `REC_BIGONE`; ASCII, ISO-8859-1, EUC-KR, and UTF-8; one-byte and `0xff` packed VARCHAR including valid compressed data and corrupt length combinations; encrypted/unreadable/missing/cyclic/unsupported evidence; and stable JSON/web projections. Prefer deterministic synthetic or pinned source-derived fixtures. Do not copy or modify the runtime oracle. Run the focused and full test suites, document any intentionally unsupported safe boundary, and commit the ticket's production test changes on `feat/page-table-attribution`.

## Comments
