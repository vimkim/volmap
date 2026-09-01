Type: task
Status: resolved
Blocked by: 02

# Promote the class-record name resolver

## Question

Can the validated `Prototype*` class-name lookup be replaced by one bounded production resolver that returns the exact stored class name or a precise typed unresolved result for the original numeric OID?

Implement exact-OID lookup through existing page/slot readers and reuse the established handling for `REC_HOME`, `REC_NEWHOME`, bounded relocation, and `REC_BIGONE` multipage-overflow records. Validate the object-representation header and variable-offset width, locate variable attribute zero, enforce the CUBRID identifier bound and terminating NUL, and decode the snapshot codeset. Packed VARCHAR handling must consume both big-endian 32-bit compressed/decompressed lengths after `0xff`; a non-zero compressed length must be safely decompressed with validated bounds or produce a typed unsupported/unresolved reason, never parsed as text. Fail closed for cycles, depth/budget stops, missing pages/slots, dead record kinds, corrupt lengths/offsets, encryption, unsupported representation/codeset, and invalid identifiers. Delete the prototype parser after production callers and tests replace it. Add focused valid and hostile-input tests, including short/long/compressed VARCHAR, all supported codesets, home/new-home, relocation, `REC_BIGONE`, and numeric-OID retention on every failure. Format, run the focused tests, and commit the ticket's production change on `feat/page-table-attribution`.

## Comments

## Answer

Implemented on `feat/page-table-attribution-current` from the prepared ticket 01–02 baseline.

The production inspection path now resolves each distinct descriptor `class_oid` once and retains either the exact stored name or a typed `ClassNameUnresolvedReason` beside the original numeric OID. `REC_HOME` and `REC_NEWHOME` consume the validated heap Page/Slot reader directly. `REC_RELOCATION` shares the same heap-owner, target-Page, and live `REC_NEWHOME` validator as relocation enrichment. `REC_BIGONE` shares heap/overflow ownership validation and one bounded, cycle-detecting overflow walker with existing enrichment; class lookup opts into transient payload capture, while published overflow facts retain only the validated structural prefix.

The new class-record decoder validates the shared object header, four-byte variable offsets, the 72-byte variable table plus the pinned 88-byte fixed region, attribute-zero bounds, OOS flags, the 255-byte identifier limit, embedded NUL, and the stored terminating NUL. Shared packed-VARCHAR handling covers short, long raw, and LZ4-compressed values, checks the decoded length before decompression allocation, and consumes both big-endian 32-bit lengths after `0xff`. Snapshot-proven ASCII, ISO-8859-1, EUC-KR, and UTF-8 decoding remains strict. Page reads admit the current physical envelope against both decoded-byte and resident-memory limits before decryption, and transient BigOne payload capture accounts for the authoritative post-read residency rather than cached TDE facts.

The prototype class-name parser and its raw offset/VARCHAR helpers were removed. The existing `page_table_poc` example is now only a production inspection consumer. Focused fixtures cover short/long/compressed names, hostile offsets and lengths, the pinned engine class Page, all supported codesets, Home/NewHome/relocation/BigOne records, rejected ownership and record roles, encrypted/decryption behavior, a live plaintext-to-encrypted state mismatch at the admission boundary, authoritative encrypted-Page residency during payload capture, resource-limited overflow prefixes, and original-OID retention for unresolved results.

Verification passed:

- `cargo fmt --all -- --check`
- `cargo clippy --locked --all-features --all-targets -- -D warnings`
- `cargo test --locked --test class_name --test class_name_resolution --test overflow_inspection --test tde_inspection` (18 passed)
- `cargo test --locked --all-features --all-targets` (all automated tests passed; three manual preview/benchmark tests remained intentionally ignored)
- `git diff --check`
- Independent Standards review: pass, zero findings
- Independent Spec review: pass, zero findings
- Read-only runtime oracle: Page `1:1000` resolved through file `1:640` (`heap-reuse-slots`) and class OID `0:209:2` to `dba.poc_table`

No new reader gap or blocker was exposed. Ticket 04 remains the next open frontier; its association-model/cache scope was not implemented here.
