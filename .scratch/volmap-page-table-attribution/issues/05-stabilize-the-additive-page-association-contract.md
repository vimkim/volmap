Type: task
Status: resolved
Blocked by: 04

# Stabilize the additive Page association contract

## Question

Can every adapter receive the same typed page association through the existing inspection/projection seam while preserving schema-version-1 JSON and HTML compatibility?

Project the production association from `PageView` into one tagged, additive `PageProjection` contract containing the allocating file identity, file role/type, numeric class OID when known, and resolved/non-applicable/unresolved class-name state with stable machine-readable reasons. Preserve all existing fields and schema version unless a concrete incompatibility proves an additive change impossible; update serialization and CLI/JSON contract fixtures accordingly. Remove `PrototypePageTableLookup`, `PrototypeTableName`, and their ad hoc query/parser surfaces. Delete `examples/page_table_poc.rs` or rewrite it only as a thin consumer of the production projection—never preserve duplicate resolution logic. Record the public contract and limitations in the appropriate repository documentation. Add escaping, omission/additivity, deterministic serialization, and backward-compatible JSON tests. Format, run the focused adapter/contract tests, and commit the ticket's production change on `feat/page-table-attribution`.

## Answer

Yes. Schema version 1 now exposes one tagged, additive `PageProjection.file_association` while preserving every prior Page field. It carries the typed file identity and role, the exact numeric class OID when available, and resolved, unresolved, or non-applicable class-name state. Unavailable states contain a stable `reason_code` for machines and a separate human-readable `reason`; existing schema-version-1 consumers may omit or ignore the additive field and code.

All finite adapters consume this shared projection: CLI JSON, the web decoder/server, HTML export, and the focused TUI contain no page-attribution parser. The former POC is a thin production-projection consumer, and no `PrototypePageTableLookup` or `PrototypeTableName` surface remains. The public compatibility and evidence limits are recorded in `docs/page-association-contract.md`.

Verification at commit `ef68458b3365f3c7ae02fd7f2a2c1c9db7c2e611` covered formatting, strict locked all-feature/all-target Clippy, the focused association/CLI/inspection suites, the full locked all-feature/all-target Rust suite, frontend type checking and 35 tests, deterministic bundle generation, browser checks, exact JSON and HTML escaping/omission fixtures, the read-only `1/1000 -> dba.poc_table` oracle, and independent Standards and Spec reviews with zero findings.

## Comments
