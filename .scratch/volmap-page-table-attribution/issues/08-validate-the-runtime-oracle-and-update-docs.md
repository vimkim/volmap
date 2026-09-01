Type: task
Status: open
Blocked by: 06, 07

# Validate the runtime oracle and update documentation

## Question

Does the production interface reproduce the validated read-only proof and leave the repository's user-facing and machine-contract documentation accurate?

Run the repository-native sample invocation against `/home/vimkim/temp/volmap/target/oos-storage-page-table-poc/db/volmap_poc_vinf` without writing to it and prove volume `1`, page `1000` resolves to `dba.poc_table` with the expected file identity/role and class OID. Exercise representative internal/unallocated and, when present, other scoped file-role pages without manufacturing names. Update README/user documentation and JSON/web contract examples to describe the Page facts, typed unresolved/non-applicable behavior, supported codesets, offline evidence requirements, and explicit sector/OOS limitations. Remove stale POC wording and ensure no production instructions depend on the mutable CUBRID worktree. Run formatting, clippy, focused tests, the full test suite, and the read-only invocation; capture exact commands/results in the ticket resolution and commit the documentation/verification fixes on `feat/page-table-attribution`.

## Comments
