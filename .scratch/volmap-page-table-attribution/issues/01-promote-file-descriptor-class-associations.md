Type: task
Status: resolved
Blocked by:

# Promote file-descriptor class associations

## Question

Can the heap-only prototype descriptor lookup be replaced with one production, source-traced descriptor association interface covering heap, heap-reuse, multipage heap overflow, B-tree, B-tree-overflow-key, extensible hash, and hash directory files without weakening existing descriptor facts?

Implement the narrow production decoder/interface at the shared file-table layer. Preserve related HFID, BTID, attribute, file-type, and VFID facts; validate the exact `class_oid` offsets documented in `docs/table-attribution-survey.md`; distinguish null/no-single-class/internal and deferred OOS cases; and add source-derived valid, null/system, short, and malformed descriptor tests for every scoped family. Remove the prototype's raw descriptor-page decoding path once its production successor is exercised. Do not add sector attribution or infer from heap/B-tree child page bytes. Format, run the focused tests, and commit the ticket's production change on `feat/page-table-attribution`.

## Comments

## Answer

Implemented and committed as `d8dad6a` (`feat: promote file descriptor class associations`) on `feat/page-table-attribution`.

The shared file-table decoder now consumes the fixed 64-byte descriptor once and retains a typed `FileDescriptor` for heap, heap-reuse, multipage object heap, B-tree, B-tree overflow-key, extensible hash, hash directory, and deferred OOS files. It preserves HFID, BTID, attribute ID, VFID, and file-type facts while exposing one normalized `FileClassAssociation`: associated OID, exact null OID, no single class, internal file, or deferred OOS. Heap descriptor self-identity validation remains fail-closed. The page/table prototype now consumes that production association and its duplicate raw descriptor-page parser was removed.

Source-derived tests exercise every scoped family through both the descriptor interface and the full file-header decoder at class-OID offsets `+0` or `+12` as appropriate. They cover valid user OIDs, the root/system OID, exact null OIDs, short descriptors, partially-null malformed OIDs, preserved HFID/BTID/attribute facts, heap identity mismatch, internal/no-single-class states, and typed OOS deferral.

Verification passed:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features` (all automated tests passed; one pre-existing manual benchmark remained ignored)
- Read-only runtime oracle: page `1:1000` resolved through file `1:640` (`heap-reuse-slots`) and class OID `0:209:2` to `dba.poc_table`.

No reader gap or other concrete follow-up was exposed, so no fog graduated and no additional ticket was created or resolved.
