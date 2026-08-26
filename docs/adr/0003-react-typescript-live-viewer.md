# Build the live viewer from pinned React and TypeScript sources

The hand-written browser adapter had kept Volmap's release graph Cargo-only, but its shared mutable navigation, polling, selection, and rendering state no longer provides a maintainable seam for cross-view byte highlighting and independently changing runtime observations. The live viewer will therefore move to React and TypeScript with an exact, locked Node build toolchain. This deliberately supersedes the earlier “no Node/npm build” decision for live web assets; the deterministic HTML inspection export remains a separate renderer.

The generated production bundle is committed and embedded in the same standalone executable. Ordinary Rust builds do not require Node, while CI and release verification regenerate the bundle and require byte-for-byte equality. The complete JavaScript build graph is license-, advisory-, source-, and lifecycle-script-audited; bundled runtime code enters product notices and the artifact SBOM, and build-only tools enter provenance. Production browser behavior is verified by reducer/selector tests and pinned headless-browser tests against the actual Rust server rather than source-text assertions alone.

## Consequences

- Adding or upgrading JavaScript dependencies requires the same deliberate review as changing the locked Cargo graph.
- The live bundle remains same-origin, CSP-compatible, offline from third-party services, and embedded in the single static executable.
- The frozen HTML export does not adopt React as part of this migration.
