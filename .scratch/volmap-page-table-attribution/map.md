Label: wayfinder:map
Status: open

# Deliver production page-to-table attribution

## Destination

A squash-ready production change on `feat/page-table-attribution` that lets a user select any physical page and see the stored CUBRID class/table name associated with its allocating file whenever validated offline evidence permits. The existing web design remains intact, the shared inspection/projection contract remains additive and fail-closed, and the branch ends with production code, tests, documentation, feature-branch commits, and successful local verification ready for the parent orchestrator to squash-merge to `main`.

## Notes

- This effort explicitly overrides Wayfinder's planning-only default. Resolving `task` tickets means implementing, testing, documenting, and committing the scoped production change on `feat/page-table-attribution`; the charting session itself creates and wires tickets only.
- Begin every implementation session with `docs/table-attribution-survey.md`, then inspect the validated `Prototype*` code in `src/inspection.rs` and `examples/page_table_poc.rs`. Promote or replace that POC with production interfaces; never leave a second parser behind.
- The canonical relationship is `Page (VPID) -> allocating File (VFID) -> descriptor class_oid -> exact class record -> stored class/table name`. An adapter consumes this shared association and never parses volume bytes or searches arbitrary page payloads for strings.
- Keep page allocation, file ownership/role, class association, and class-name resolution as distinct facts. Retain a numeric OID when name resolution fails. Resolve each distinct OID once and join normalized maps at the inspection/projection seam instead of copying a `String` into every compact physical-page fact.
- Fail closed on incomplete inventory, conflicting ownership, corruption, unreadable or encrypted evidence, unsupported representation or codeset, and inconsistent volume codesets. Catalog, global, internal, unallocated, and reserved-unallocated pages must never be labeled as user tables.
- Scoped file descriptors are heap, heap-reuse, multipage heap overflow, B-tree, B-tree-overflow-key, extensible hash, and hash directory. `FILE_OOS` is explicitly deferred and must yield a typed non-applicable/unresolved state, never an inferred class name.
- Page ownership is the only product scope. Do not promote the survey's sector-attribution proposal.
- Production class-record resolution must reuse existing bounded page/slot, relocation, and multipage-overflow readers for `REC_HOME`, `REC_NEWHOME`, bounded `REC_RELOCATION`, and `REC_BIGONE`. Packed VARCHAR `0xff` handling reads both 32-bit lengths and never treats compressed bytes as text. Supported identifier codesets are ASCII, ISO-8859-1, EUC-KR, and UTF-8, with a precise typed unsupported result where safe decoding is unavailable.
- Add `Class/table`, `Class OID`, `File`, and `File role` to the existing web Page facts. Preserve the current layout and stable JSON/HTML behavior; schema changes are additive unless a concrete compatibility violation proves otherwise.
- At chart time, `prototype/tui-web-parity` is not an ancestor of `main` (`main` and this branch start at `95b4b69`). Re-check immediately before final integration. Only if parity is actually present in this branch's then-current `main` baseline should the TUI Page view consume the shared association, without parser duplication.
- Reconciliation snapshot (2026-09-01): current `main` at `64a9dd3` contains the original production attribution change (`cba72cd`) and the focused TUI (`f44fec5`), so ticket 09's parity condition is now true. It does not contain patch-equivalent versions of the two resolved hardening commits `d8dad6a` and `4776865` (`git cherry main feat/page-table-attribution` reports both as unique). The map therefore remains open at ticket 03. Resume from a fresh worktree based on then-current `main`, port and re-verify tickets 01-02, and continue the unresolved frontier there; do not treat the old divergent checkout as an integration base.
- Runtime oracle: `/home/vimkim/temp/volmap/target/oos-storage-page-table-poc/db/volmap_poc_vinf`, generated from `/home/vimkim/gh/cb/oos-storage` on `feat/oos`. Keep it read-only. The known proof is volume `1`, page `1000` -> `dba.poc_table`.
- Use repository-native Rust formatting, clippy, tests, and the read-only sample invocation. For minor details, preserve current behavior and select the narrow fail-closed option; no further product choice is required.
- Preserve external dirty work. Do not touch `.scratch/volmap-tui-web-parity`, `/home/vimkim/temp/volmap-tui-web-parity-prototype`, the CUBRID source worktree, unrelated files, or unrelated branches. The parent orchestrator owns the final squash merge to `main`.
- Durable work-tracker item: `15`.

## Decisions so far

- [Promote file-descriptor class associations](issues/01-promote-file-descriptor-class-associations.md) — One source-pinned descriptor decoder now retains typed file relationships and explicit class-association states for every scoped family.
- [Retain and validate the database codeset](issues/02-retain-and-validate-database-codeset.md) — Volume records retain raw header values, the linked inventory proves one typed snapshot codeset, and bounded strict decoders cover ASCII, ISO-8859-1, EUC-KR, and UTF-8 without replacement text.

## Not yet specified

- Exact follow-up work, if any, exposed when the production resolver exercises existing bounded relocation or `REC_BIGONE` readers against class records. Graduate only concrete reader gaps; do not pre-authorize a parallel class-record traversal stack.
- Exact TUI touchpoints beyond the shared Page projection, if the parity implementation becomes part of `main` before final integration. If it remains absent, this fog expires without an implementation ticket.
- Targeted fixture or diagnostic additions exposed by the read-only runtime oracle or full verification after the currently specified hostile-input matrix is complete.

## Out of scope

- Sector table/class attribution, sector-card labels, reserved-sector ownership display, and mixed-sector semantics.
- OOS table attribution. `FILE_OOS` receives only an explicit typed deferred/non-applicable or unresolved result in this effort.
- Catalog-record class annotations, a general class browser, root-class heap scans, classname extensible-hash traversal, arbitrary byte/string scanning, SQL sidecars, or a runtime CUBRID/`diagdb` product dependency.
- Redesigning the web UI, adding a new adapter-specific parser, or changing unrelated CLI/TUI behavior.
- Inspecting or mutating a running database, modifying the read-only oracle, or changing the CUBRID source worktree.
- Touching the unrelated parity scratch/worktree or integrating unrelated branches and dirty work.
- Performing the final squash merge to `main`; the parent orchestrator performs it after this map reaches the destination.
