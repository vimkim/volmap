Type: task
Status: open
Blocked by: 08

# Re-check TUI parity and finalize the branch

## Question

Is the then-current `main` baseline TUI/web-parity implementation present, and is `feat/page-table-attribution` production-complete and squash-ready under that answer?

Immediately before final integration, verify whether `prototype/tui-web-parity` (or its merged equivalent) is an ancestor of the branch's then-current `main` baseline. If parity is absent, keep TUI changes out of scope and record the evidence. If parity is present, integrate/rebase the authorized main baseline as required by the parent workflow and make its TUI Page view consume the existing shared Page association—no TUI parser, cache, or alternate semantics—and add parity-focused tests without redesign. In either case, audit the final diff for leftover `Prototype*`/POC paths, sector attribution, inferred OOS names, duplicated strings in compact page facts, unrelated changes, and accidental edits to protected scratch/worktrees or the CUBRID source tree. Run repository-native Rust formatting, clippy, all tests, relevant contract/web tests, and the read-only oracle invocation. Ensure one or more coherent feature-branch commits exist and the branch is ready for the parent orchestrator's squash merge; do not perform that squash merge. Record exact baseline/verification evidence in the ticket resolution.

## Comments
