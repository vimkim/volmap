Type: task
Status: resolved
Blocked by: 06, 07

# Validate the runtime oracle and update documentation

## Question

Does the production interface reproduce the validated read-only proof and leave the repository's user-facing and machine-contract documentation accurate?

Run the repository-native sample invocation against `/home/vimkim/temp/volmap/target/oos-storage-page-table-poc/db/volmap_poc_vinf` without writing to it and prove volume `1`, page `1000` resolves to `dba.poc_table` with the expected file identity/role and class OID. Exercise representative internal/unallocated and, when present, other scoped file-role pages without manufacturing names. Update README/user documentation and JSON/web contract examples to describe the Page facts, typed unresolved/non-applicable behavior, supported codesets, offline evidence requirements, and explicit sector/OOS limitations. Remove stale POC wording and ensure no production instructions depend on the mutable CUBRID worktree. Run formatting, clippy, focused tests, the full test suite, and the read-only invocation; capture exact commands/results in the ticket resolution and commit the documentation/verification fixes on `feat/page-table-attribution`.

## Answer

Yes. The renamed thin production example reproduced the canonical proof:

```sh
cargo run --locked --quiet --example page_file_association -- \
  --vinf /home/vimkim/temp/volmap/target/oos-storage-page-table-poc/db/volmap_poc_vinf \
  1 1000
```

Page `1:1000` is allocated by File `1:640`, whose role is
`heap-reuse-slots`, whose descriptor retains class OID `0:209:2`, and whose
exact stored class name resolves to `dba.poc_table`.

A full production `volmap map` projection of the same immutable snapshot
covered 12,288 Pages: 1,021 allocated, 4,479 reserved-unallocated, four system
metadata, and 6,784 unreserved. It retained 1,021 allocated associations,
4,355 `reserved-for` associations, and 6,912 `none` associations. Representative
evidence remained fail-closed: Page `0:2` was reserved-unallocated with no
association; catalog Page `0:576` was `not-applicable` with
`class-association.internal-file`; B-tree Page `0:896` resolved its exact OID
to `_db_user`; and extensible-hash Page `0:320` retained OID `0:193:1` while
publishing `class.name.var_table_order` rather than manufacturing a name. The
snapshot also contained heap, heap-reuse, hash-directory, and internal file
roles. It contained no multipage-heap-overflow, B-tree-overflow-key, or OOS
role, so no claim was manufactured for those absent cases.

The aggregate and representative evidence came from this exact read-only
command (the `jq` filter emits counts plus Pages `0:2`, `0:320`, `0:576`,
`0:896`, and `1:1000`):

```sh
oracle_vinf=/home/vimkim/temp/volmap/target/oos-storage-page-table-poc/db/volmap_poc_vinf
cargo run --locked --quiet -- map --vinf "$oracle_vinf" \
  --format json --progress never |
  jq '{
    allocation_counts: ([.data.sectors[].pages[].allocation] |
      group_by(.) | map({state: .[0], count: length})),
    association_counts: ([.data.sectors[].pages[].file_association.state] |
      group_by(.) | map({state: .[0], count: length})),
    file_roles: ([.data.sectors[].pages[] |
      select(.file_association.file != null) |
      .file_association.file.file_type.value] |
      group_by(.) | map({role: .[0], count: length})),
    representative_pages: [.data.sectors[].pages[] |
      select((.vol_id == 0 and
        (.page_id == 2 or .page_id == 320 or .page_id == 576 or
         .page_id == 896)) or
        (.vol_id == 1 and .page_id == 1000)) |
      {vpid: [.vol_id, .page_id], allocation,
       association: .file_association}]
  }'
```

The input remained byte-identical. Before and after the read-only runs,
SHA-256 was `b2f7601392a42e481e2c1fd3529a657d3213f96284f0872fde56b006813b077b`
for the VINF,
`f10dcfb141d6bfdd807a261c5700f8bcdb6d62546a1c59eb5b26e1daa99432d2`
for volume 0, and
`63be7b5de4f49b081446c7ee90bd16299a17a21b5bfb4a2db7b7b25b80a840ac`
for volume 1; sizes and modification/change times were also unchanged.

The exact integrity check wrapped a production read between identical
SHA-256/stat snapshots and required byte-for-byte shell equality:

```sh
oracle_vinf=/home/vimkim/temp/volmap/target/oos-storage-page-table-poc/db/volmap_poc_vinf
oracle_dir=${oracle_vinf%/*}
oracle_integrity() {
  sha256sum "$oracle_vinf" "$oracle_dir/volmap_poc" \
    "$oracle_dir/volmap_poc_x001"
  stat --format='%n\t%s\t%Y\t%Z' "$oracle_vinf" \
    "$oracle_dir/volmap_poc" "$oracle_dir/volmap_poc_x001"
}
oracle_integrity_before=$(oracle_integrity)
cargo run --locked --quiet --example page_file_association -- \
  --vinf "$oracle_vinf" 1 1000
oracle_integrity_after=$(oracle_integrity)
test "$oracle_integrity_before" = "$oracle_integrity_after"
printf '%s\n' "$oracle_integrity_after"
```

`README.md` and `docs/page-association-contract.md` now describe the shared
Page facts, JSON resolved/unresolved/not-applicable shapes, stable reason-code
behavior, four supported codesets, offline evidence requirements, and the
sector/OOS boundaries. The source survey is explicitly marked historical. The
stale POC-named example became `examples/page_file_association.rs`, and its
developer recipe now invokes that production projection consumer. No product
instruction names the external oracle or depends on a mutable CUBRID worktree.

Verification passed these exact repository commands:

```sh
cargo fmt --all -- --check
cargo test --locked --all-features --example page_file_association
cargo test --locked --all-features --test page_association_contract --test class_name_resolution --test cli_contract
cargo clippy --locked --all-features --all-targets -- -D warnings
cargo test --locked --all-features --all-targets
just vite::frontend-check
```

The focused Rust suites passed 20 tests. The full Rust suite passed with three
intentional manual-test ignores. Frontend verification passed TypeScript,
seven files and 38 unit tests, deterministic asset and advisory checks, and the
real-server Chromium/Firefox run (three passed, one intentionally skipped).

## Comments
