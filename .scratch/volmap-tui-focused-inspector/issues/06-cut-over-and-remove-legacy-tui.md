# 06: Cut over and remove the legacy TUI

**What to build:** Replace the production `tui::run` path with the focused Volume → Sector → Page inspector after all focused evidence passes, then delete the legacy fixed-coordinate state, tab, and draw path. Follow F5 in the [focused TUI implementation specification](../implementation-spec.md).

**Blocked by:** [05: Harden the terminal session](05-harden-terminal-session.md).

**Status:** implemented

**Assignee:** codex

- [x] The CLI passes an owned initial `GraphView` and bounded `ResourcePolicy` into the TUI and receives a typed exit carrying the final adopted view.
- [x] Final CLI outcome and exit handling use the returned view rather than the pre-TUI overview, so successful enrichment or invalidation cannot leave status stale.
- [x] Production startup opens Volume mode and exposes only the accepted three persistent modes plus Page-local record interpretation and the small contextual key legend.
- [x] Legacy single-Sector state, fixed row hit testing, utility tabs/placeholders, subject-string finding navigation, and duplicate truncation/render paths are removed rather than layered beneath the new session.
- [x] No Projection workspace, Atlas protocol, HTTP/server handoff, browser launcher, Ratatui dependency, or superseded full-parity implementation is introduced.
- [x] Web, React, CLI, JSON/JSONL, deterministic HTML, inspection, record interpretation, disclosure, static-musl, notice, SBOM, and reproducibility regressions all pass on one candidate.
- [x] The final production diff has no dormant legacy TUI implementation, all focused acceptance evidence is checked in and reproducible, and user-facing command/help behavior matches the focused product boundary.

## Resolution

Production now enters the focused Volume → Sector → Page terminal session directly. The CLI supplies the owned initial view and bounded resource policy, derives its final outcome from the returned adopted view, and the legacy fixed-coordinate implementation has been deleted. The scripted-host, real CLI-through-PTY, semantic-golden, storage-adapter, static-musl, React/browser, notice, SBOM, and reproducibility gates all pass on the same candidate.
