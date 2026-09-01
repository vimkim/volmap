# Repository Guidelines

## Project Structure & Module Organization

Volmap is a Rust 2024 read-only CUBRID volume inspector. Core code lives in `src/`: format decoders are under `src/format/`, presentation adapters include `cli.rs`, `tui.rs`, and `web.rs`, and `src/main.rs` defines the binary. Rust integration tests live in `tests/`; reusable sample volumes and manifests are in `fixtures/`. The React/TypeScript viewer is maintained in `web/src/`, with browser tests in `web/e2e/`. Design decisions and format contracts belong in `docs/` and `docs/adr/`. Treat `src/web/generated/` and release supply-chain files as generated artifacts.

## Build, Test, and Development Commands

Use the repository `just` recipes, which pin Rust 1.97.1 and frontend tooling:

- `just build-debug` builds the Rust binary with the locked dependency graph.
- `just test-debug` runs Rust unit, integration, and documentation tests.
- `just fmt` formats Rust; `just lint` runs Clippy for all targets and features with warnings denied.
- `just vite::frontend-generate-artifacts` rebuilds committed JS/CSS after changes under `web/src/`.
- `just frontend-check` runs frontend types, Vitest, artifact checks, advisories, and Playwright coverage.
- `just verify` runs all local pre-commit gates, including the static-musl release check.

Run the CLI during development with `just run-debug -- <arguments>` or inspect available recipes with `just --list`.

## Coding Style & Naming Conventions

Accept `rustfmt` defaults (four-space indentation) and keep code Clippy-clean. Rust modules, functions, and tests use `snake_case`; types and traits use `UpperCamelCase`. TypeScript uses two spaces, double quotes, semicolons, `camelCase` values, and `PascalCase` React components. Keep format parsing in `src/format/` and project shared inspection facts through adapters instead of reparsing bytes in UI layers. Preserve Volmap's read-only and explicit-disclosure guarantees described in `README.md` and `CONTEXT.md`.

## Testing Guidelines

Add focused Rust integration tests as `tests/<feature>.rs` with descriptive `#[test]` names. Frontend unit tests use `web/src/*.test.ts(x)` with Vitest; browser contracts use `web/e2e/*.spec.ts` with Playwright. Update `tests/goldens/` only when terminal rendering intentionally changes. Prefer existing pinned fixtures; document any new corpus generation steps.

## Commit & Pull Request Guidelines

Recent history favors imperative Conventional Commit subjects such as `feat:`, `feat(tui):`, `docs:`, `build:`, and `web:`. Keep each commit scoped and explain behavioral or contract changes in its body. Pull requests should state purpose and verification, link the relevant issue or ADR, call out generated artifacts, and include screenshots for visible TUI or web changes. Run `just verify` before requesting review.
