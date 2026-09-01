Type: task
Status: resolved
Blocked by: 05

# Complete the production attribution test matrix

## Question

Does the integrated production path have enough source-derived and hostile-input coverage to prove all scoped file roles, record shapes, codesets, and fail-closed boundaries without relying on a mutable external database as a committed golden?

Audit the tests landed with the decoder, resolver, model, and projection tickets; add only the missing fixtures and integration cases. Cover heap/heap-reuse, multipage heap overflow, B-tree, B-tree-overflow-key, extensible hash, and hash directory descriptors; catalog/global/internal/unallocated/reserved-unallocated and deferred OOS non-attribution; complete versus incomplete inventory; distinct-OID single resolution; compact page-fact size; home/new-home, bounded relocation, and `REC_BIGONE`; ASCII, ISO-8859-1, EUC-KR, and UTF-8; one-byte and `0xff` packed VARCHAR including valid compressed data and corrupt length combinations; encrypted/unreadable/missing/cyclic/unsupported evidence; and stable JSON/web projections. Prefer deterministic synthetic or pinned source-derived fixtures. Do not copy or modify the runtime oracle. Run the focused and full test suites, document any intentionally unsupported safe boundary, and commit the ticket's production test changes on `feat/page-table-attribution`.

## Answer

Yes. The audited production matrix now covers every scoped file family, normalized single-resolution sharing, the exact 16-byte compact Page fact, allocation and non-attribution states, complete and structurally incomplete inventories, all supported database codesets, Home/NewHome/relocation/BigOne records, and the stable Page JSON, React, and frozen-HTML projections. Existing source-derived and synthetic fixtures already supplied most of that coverage, so this ticket changed tests only.

The missing hostile boundaries are now deterministic fixtures. Class-name decoding rejects negative, truncated, oversized, and invalid-compressed `0xff` VARCHAR length combinations before unsafe allocation. Integrated resolution retains the original typed OID while reporting opaque encryption, an unavailable Page, a missing Slot, a second acyclic relocation, a relocation self-cycle, a cyclic BigOne overflow chain, wrong ownership, dead records, invalid Page roles, and unsupported codeset/identifier evidence. System metadata with conflicting reservation claims remains system metadata and never promotes one user class.

The intentionally unsupported boundaries remain fail-closed: class attribution follows exactly one validated relocation whose target must be `REC_NEWHOME`; an out-of-row class-name attribute is rejected rather than traversed; encrypted class pages require usable TDE evidence; and OOS file attribution remains explicitly deferred. The external sample database was read only as an optional oracle and was not copied or committed as a golden.

Verification passed repository formatting, strict locked all-feature/all-target Clippy, focused descriptor/header/class-name/association/CLI/compact-fact suites, the full locked all-feature/all-target Rust suite, frontend type checking and 38 tests, deterministic bundle and supply-chain checks, Chromium and Firefox checks against the Rust server, and the read-only `1/1000 -> dba.poc_table` oracle. Independent Standards and Spec reviews found no remaining issue.

## Comments
