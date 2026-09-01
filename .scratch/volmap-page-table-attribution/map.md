Label: wayfinder:map
Status: resolved

# Deliver production page-to-table attribution

## Destination

A squash-ready production change on `feat/page-table-attribution` that lets a user select any physical page and see the stored CUBRID class/table name associated with its allocating file whenever validated offline evidence permits. The existing web design remains intact, the shared inspection/projection contract remains additive and fail-closed, and the branch ends with production code, tests, documentation, feature-branch commits, and successful local verification ready for the parent orchestrator to squash-merge to `main`.

## Notes

- This effort explicitly overrides Wayfinder's planning-only default. Resolving `task` tickets means implementing, testing, documenting, and committing the scoped production change on `feat/page-table-attribution`; the charting session itself creates and wires tickets only.
- Begin every implementation session with `docs/table-attribution-survey.md`, then inspect the production resolver in `src/inspection.rs` and its thin consumer in `examples/page_file_association.rs`. Keep one shared resolver; never add an adapter-specific parser.
- The canonical relationship is `Page (VPID) -> allocating File (VFID) -> descriptor class_oid -> exact class record -> stored class/table name`. An adapter consumes this shared association and never parses volume bytes or searches arbitrary page payloads for strings.
- Keep page allocation, file ownership/role, class association, and class-name resolution as distinct facts. Retain a numeric OID when name resolution fails. Resolve each distinct OID once and join normalized maps at the inspection/projection seam instead of copying a `String` into every compact physical-page fact.
- Fail closed on incomplete inventory, conflicting ownership, corruption, unreadable or encrypted evidence, unsupported representation or codeset, and inconsistent volume codesets. Catalog, global, internal, unallocated, and reserved-unallocated pages must never be labeled as user tables.
- Scoped file descriptors are heap, heap-reuse, multipage heap overflow, B-tree, B-tree-overflow-key, extensible hash, and hash directory. `FILE_OOS` is explicitly deferred and must yield a typed non-applicable/unresolved state, never an inferred class name.
- Page ownership is the only product scope. Do not promote the survey's sector-attribution proposal.
- Production class-record resolution must reuse existing bounded page/slot, relocation, and multipage-overflow readers for `REC_HOME`, `REC_NEWHOME`, bounded `REC_RELOCATION`, and `REC_BIGONE`. Packed VARCHAR `0xff` handling reads both 32-bit lengths and never treats compressed bytes as text. Supported identifier codesets are ASCII, ISO-8859-1, EUC-KR, and UTF-8, with a precise typed unsupported result where safe decoding is unavailable.
- Add `Class/table`, `Class OID`, `File`, and `File role` to the existing web Page facts. Preserve the current layout and stable JSON/HTML behavior; schema changes are additive unless a concrete compatibility violation proves otherwise.
- At chart time, `prototype/tui-web-parity` is not an ancestor of `main` (`main` and this branch start at `95b4b69`). Re-check immediately before final integration. Only if parity is actually present in this branch's then-current `main` baseline should the TUI Page view consume the shared association, without parser duplication.
- Reconciliation snapshot (2026-09-01): current `main` at `64a9dd3` contains the original production attribution change (`cba72cd`) and the focused TUI (`f44fec5`), so ticket 09's parity condition is now true. It does not contain patch-equivalent versions of the two resolved hardening commits `d8dad6a` and `4776865` (`git cherry main feat/page-table-attribution` reports both as unique). The map therefore remains open at ticket 03. Resume from a fresh worktree based on then-current `main`, port and re-verify tickets 01-02, and continue the unresolved frontier there; do not treat the old divergent checkout as an integration base.
- Preparation snapshot (2026-09-01): `feat/page-table-attribution-current` was created from `origin/main` at `ece9944`. The intent of `d8dad6a` and `4776865` was reconciled onto the evolved code instead of blindly cherry-picked: current live-follow source modes, record interpretation, slotted-distribution eligibility, focused TUI, and adapter behavior remain in place while the typed descriptor model and snapshot-validated database codeset are restored. Focused file-table, volume-header, inspection, formatting, Clippy, and full-suite verification belong to this preparation commit. No ticket 03 behavior is included; ticket 03 remains the first open, unblocked frontier.
- Runtime oracle: `/home/vimkim/temp/volmap/target/oos-storage-page-table-poc/db/volmap_poc_vinf`, generated from `/home/vimkim/gh/cb/oos-storage` on `feat/oos`. Keep it read-only. The known proof is volume `1`, page `1000` -> `dba.poc_table`.
- Use repository-native Rust formatting, clippy, tests, and the read-only sample invocation. For minor details, preserve current behavior and select the narrow fail-closed option; no further product choice is required.
- Preserve external dirty work. Do not touch `.scratch/volmap-tui-web-parity`, `/home/vimkim/temp/volmap-tui-web-parity-prototype`, the CUBRID source worktree, unrelated files, or unrelated branches. The parent orchestrator owns the final squash merge to `main`.
- Durable work-tracker item: `15`.

## Decisions so far

- [Promote file-descriptor class associations](issues/01-promote-file-descriptor-class-associations.md) — One source-pinned descriptor decoder now retains typed file relationships and explicit class-association states for every scoped family.
- [Retain and validate the database codeset](issues/02-retain-and-validate-database-codeset.md) — Volume records retain raw header values, the linked inventory proves one typed snapshot codeset, and bounded strict decoders cover ASCII, ISO-8859-1, EUC-KR, and UTF-8 without replacement text.
- [Promote the class-record name resolver](issues/03-promote-the-class-record-name-resolver.md) — Exact descriptor OIDs now resolve through shared bounded Home/NewHome, relocation, and multipage-overflow readers to a strict stored class name or a typed unresolved reason, with no duplicate prototype parser.
- [Build normalized page-file-class associations](issues/04-build-normalized-page-file-class-associations.md) — Complete validated inventory is the only class-promotion boundary; normalized page/file/OID maps join typed resolved, unresolved, and non-applicable facts into `PageView` without growing the 16-byte packed page fact.
- [Stabilize the additive Page association contract](issues/05-stabilize-the-additive-page-association-contract.md) — Schema-version-1 Page projections now expose one tagged additive association with stable machine reason codes, and every finite adapter consumes that shared contract without a duplicate parser.
- [Add association to the web Page facts](issues/06-add-association-to-web-page-facts.md) — The live React and frozen HTML Page panels now render the shared File, File role, Class OID, and Class/table facts across resolved and fail-closed states without changing navigation or Sector presentation.
- [Complete the production attribution test matrix](issues/07-complete-the-production-attribution-test-matrix.md) — Deterministic source-derived and hostile fixtures now cover every scoped descriptor, record/codeset shape, normalized association, adapter contract, and fail-closed boundary without committing the mutable runtime oracle.
- [Validate the runtime oracle and update documentation](issues/08-validate-the-runtime-oracle-and-update-docs.md) — The read-only production projection proves Page `1:1000` maps through File `1:640` and OID `0:209:2` to `dba.poc_table`; user and machine-contract documentation now records the exact evidence, codeset, sector, and OOS boundaries without a POC-named product path.
- [Re-check TUI parity and finalize the branch](issues/09-recheck-tui-parity-and-finalize-the-branch.md) — Current `origin/main` contains the focused TUI and is the feature branch's exact merge base; the TUI consumes every shared Page association state, and the audited, fully verified branch is ready for its parent-owned squash merge.

## Not yet specified

No remaining fog. The destination is reached.

## Out of scope

- Sector table/class attribution, sector-card labels, reserved-sector ownership display, and mixed-sector semantics.
- OOS table attribution. `FILE_OOS` receives only an explicit typed deferred/non-applicable or unresolved result in this effort.
- Catalog-record class annotations, a general class browser, root-class heap scans, classname extensible-hash traversal, arbitrary byte/string scanning, SQL sidecars, or a runtime CUBRID/`diagdb` product dependency.
- Redesigning the web UI, adding a new adapter-specific parser, or changing unrelated CLI/TUI behavior.
- Inspecting or mutating a running database, modifying the read-only oracle, or changing the CUBRID source worktree.
- Touching the unrelated parity scratch/worktree or integrating unrelated branches and dirty work.
- Performing the final squash merge to `main`; the parent orchestrator performs it after this map reaches the destination.
