Type: task
Status: open
Blocked by: 04

# Stabilize the additive Page association contract

## Question

Can every adapter receive the same typed page association through the existing inspection/projection seam while preserving schema-version-1 JSON and HTML compatibility?

Project the production association from `PageView` into one tagged, additive `PageProjection` contract containing the allocating file identity, file role/type, numeric class OID when known, and resolved/non-applicable/unresolved class-name state with stable machine-readable reasons. Preserve all existing fields and schema version unless a concrete incompatibility proves an additive change impossible; update serialization and CLI/JSON contract fixtures accordingly. Remove `PrototypePageTableLookup`, `PrototypeTableName`, and their ad hoc query/parser surfaces. Delete `examples/page_table_poc.rs` or rewrite it only as a thin consumer of the production projection—never preserve duplicate resolution logic. Record the public contract and limitations in the appropriate repository documentation. Add escaping, omission/additivity, deterministic serialization, and backward-compatible JSON tests. Format, run the focused adapter/contract tests, and commit the ticket's production change on `feat/page-table-attribution`.

## Comments
