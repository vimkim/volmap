Type: task
Status: resolved
Blocked by:

# Retain and validate the database codeset

## Question

Can the database codeset already decoded from each volume header become a trustworthy snapshot-level production fact for class-name decoding?

Preserve the header codeset through `VolumeRecord`/inspection construction, require consistent supported values across the complete volume inventory, and expose precise typed failure when evidence is incomplete, inconsistent, corrupt, or unsupported. Establish bounded identifier decoding for ASCII, ISO-8859-1, EUC-KR, and UTF-8 using the repository's dependency and safety conventions; do not assume UTF-8 or silently replace invalid sequences. Add focused tests for every supported codeset, invalid sequences, unsupported numeric codes, inconsistent volumes, and incomplete inventory. Keep paths and unrelated volume-header strings out of projections. Format, run the focused tests, and commit the ticket's production change on `feat/page-table-attribution`.

## Comments

## Answer

Implemented and committed as `4776865` (`feat: retain validated database codeset`) on `feat/page-table-attribution`.

Every decoded volume header now retains its raw codeset on `VolumeRecord`, and inspection construction validates the complete `next_vol_id` chain before promoting one supported `DatabaseCodeset` into the immutable snapshot. The public inspection seam returns a typed `DatabaseCodesetFailure` for incomplete inventory, inconsistent volume values, or an unsupported numeric value; corrupt volume headers continue to stop construction with the existing typed `OpenFailure::Format`, so no impossible “corrupt but constructed” snapshot state was introduced. The codeset contributes to snapshot identity, and neither it nor unrelated header strings or paths were added to projections.

Class-name decoding now consumes only that snapshot fact. ASCII, ISO-8859-1, EUC-KR, and UTF-8 are supported with a 255-byte identifier bound and embedded-NUL rejection. EUC-KR uses pinned `encoding_rs` `0.8.35` through its no-replacement decoder, while UTF-8 and ASCII remain strict; malformed sequences are returned as unresolved errors instead of replacement text.

Focused tests cover every supported header value, a complete linked multi-volume inventory, missing and broken inventory chains, inconsistent volume values, unsupported numeric codes, valid non-ASCII ISO-8859-1/EUC-KR/UTF-8 identifiers, malformed ASCII/EUC-KR/UTF-8, empty/oversized identifiers, and embedded NUL. Construction-level coverage proves the retained snapshot getter, and existing hostile volume-header tests prove corrupt headers fail before construction.

Verification passed:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test inspection::tests:: --lib`
- `cargo test --test volume_header`
- `cargo test --test inspection_scan inspection_opens_sparse_volume_and_scans_only_reserved_sector_envelopes`
- `cargo test --all-targets --all-features` (all automated tests passed; one pre-existing manual resource benchmark remained ignored)
- Read-only runtime oracle: page `1:1000` still resolved through file `1:640` (`heap-reuse-slots`) and class OID `0:209:2` to `dba.poc_table`; its real two-volume header chain validated as complete and consistent.

No concrete reader gap or other follow-up was exposed, so no fog graduated and no additional ticket was created or resolved.
