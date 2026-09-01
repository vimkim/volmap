Type: task
Status: resolved
Blocked by: 08

# Re-check TUI parity and finalize the branch

## Question

Is the then-current `main` baseline TUI/web-parity implementation present, and is `feat/page-table-attribution` production-complete and squash-ready under that answer?

Immediately before final integration, verify whether `prototype/tui-web-parity` (or its merged equivalent) is an ancestor of the branch's then-current `main` baseline. If parity is absent, keep TUI changes out of scope and record the evidence. If parity is present, integrate/rebase the authorized main baseline as required by the parent workflow and make its TUI Page view consume the existing shared Page association—no TUI parser, cache, or alternate semantics—and add parity-focused tests without redesign. In either case, audit the final diff for leftover `Prototype*`/POC paths, sector attribution, inferred OOS names, duplicated strings in compact page facts, unrelated changes, and accidental edits to protected scratch/worktrees or the CUBRID source tree. Run repository-native Rust formatting, clippy, all tests, relevant contract/web tests, and the read-only oracle invocation. Ensure one or more coherent feature-branch commits exist and the branch is ready for the parent orchestrator's squash merge; do not perform that squash merge. Record exact baseline/verification evidence in the ticket resolution.

## Answer

Yes. Immediately before finalization, `git fetch --prune origin` left
`origin/main` at `ece99446cac7c92f8fd7a29b039b7e65820c7e8f`. That baseline contains
the focused TUI cutover commit `f44fec5`; `git branch -r --contains f44fec5`
listed `origin/main`. The exact merge base of this feature branch and
`origin/main` was also `ece9944`, and `git rev-list --left-right --count
HEAD...origin/main` reported `7 0`. The branch was therefore already based on
the current authorized parity baseline, with no merge or rebase required.

The focused TUI already consumed the shared `PageProjection.file_association`
through `PageMark::try_from_projection` and rendered it through
`PageAttributionMark::labels`; this branch's earlier compatibility change only
accepted the additive machine `reason_code`. No TUI parser, cache, storage
reader, or alternate association was present. This ticket added a focused
projection-to-renderer regression for unowned, mixed, allocated/resolved,
reserved/unresolved with retained OID, internal/not-applicable, and
deferred-OOS states. It proves the Page view preserves the shared relationship
and display reason without inferring a name.

The final audit found no production `Prototype*`, POC example/command, or
adapter-specific attribution parser. The prior survey's generic “POC” wording
was removed. The feature diff adds no new Sector UI behavior; existing Sector
claim summaries came from the authorized baseline and remain separate from the
Page contract. `FILE_OOS` still maps only to
`class-association.oos-deferred`, and the TUI regression renders that typed
state without a name. `PackedPageFact` remains exactly 16 bytes, with normalized
File/OID/name maps joined below the compact scan fact. The feature diff contains
no protected parity scratch path, and this session did not edit the external
parity or CUBRID worktrees.

These final commands passed:

```sh
git fetch --prune origin
git merge-base HEAD origin/main
git rev-list --left-right --count HEAD...origin/main
git branch -r --contains f44fec5
cargo test --locked --all-features \
  tui::focused::tests::page_view_consumes_every_shared_association_state_without_inference
cargo test --locked --all-features --all-targets
just verify
mise x node@24.19.0 -- just release-audit
```

The full all-feature/all-target suite passed with 106 library tests and three
intentional manual-test ignores across the complete run. `just verify` passed
formatting, the default full Rust and doc suites, strict all-feature/all-target
Clippy, the optimized static-musl ELF checks, locked metadata and release-path
checks, TypeScript, seven frontend files and 38 tests, deterministic generated
assets, advisories, and the real-server Chromium/Firefox checks (three passed,
one intentionally skipped).

The clean-commit release audit additionally passed deterministic frontend
regeneration, dependency policy, notices and SBOM reproduction, two independent
static-musl builds with byte-identical binaries, static ELF checks, and the
complete optimized release test suite. The pinned `mise` invocation is required
because the ambient shell's Node 25 does not satisfy the repository's exact
Node 24.19.0 release-toolchain contract.

The exact read-only oracle command also passed:

```sh
cargo run --locked --quiet --example page_file_association -- \
  --vinf /home/vimkim/temp/volmap/target/oos-storage-page-table-poc/db/volmap_poc_vinf \
  1 1000
```

It still resolved Page `1:1000` through File `1:640`
(`heap-reuse-slots`) and OID `0:209:2` to `dba.poc_table`. Before/after
SHA-256 and size/mtime/ctime snapshots were identical for the VINF and both
permanent volumes. The branch has coherent prerequisite, resolver, normalized
association, contract, web, matrix, documentation, and final-parity commits.
It is ready for the parent orchestrator's squash merge; this ticket did not
push or merge it.

## Comments
