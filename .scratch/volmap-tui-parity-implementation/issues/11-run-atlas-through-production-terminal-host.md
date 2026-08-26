# 11: Run Atlas through the production Crossterm host

**What to build:** Run the complete non-production Atlas experience through the repository-owned Crossterm terminal host and real pseudo-terminals. The host owns terminal mechanics and scheduling while semantic behavior remains in AtlasMachine, Projection workspace, and AtlasRenderer.

**Blocked by:** 05: Navigate complete Volumes within bounded resources; 08: Restore selectors, filters, findings, and utility regions; 10: Close enrichment cancellation and revision races.

**Status:** superseded

**Superseded by:** [Volmap focused TUI implementation specification](../../volmap-tui-focused-inspector/implementation-spec.md).

- [ ] The host owns TTY validation, profile selection, raw mode, alternate screen, cursor, mouse capture, normalized key/mouse/resize delivery, frame presentation, and complete teardown.
- [ ] Partial entry and every normal or typed-error exit restore exactly the terminal capabilities that were acquired; a scoped best-effort panic hook covers the release build's abort behavior.
- [ ] The event loop keeps one dirty frame, preserves all ordered input, drains at most the accepted ready-input bound, coalesces resize and newest-valid progress, applies input-first ordering, and performs no idle redraw.
- [ ] Interactive, progress, completion, cancellation, and fault scheduling follows the accepted cadence without allowing presentation to initiate inspection work.
- [ ] Only successful full presentation advances the previous-frame cache and returns a matching layout commit; short write, flush failure, and stale generation cannot install geometry.
- [ ] Scripted-host tests cover deterministic scheduling and injected faults without sleeps; exact-version Expect PTY tests cover real Crossterm input, resize, entry, cancellation during quit, panic cleanup, and restoration.
- [ ] Production uses the pinned Crossterm dependency and accepted exact Unicode text dependencies only; Ratatui and generic terminal-widget/runtime protocols are absent.
- [ ] Atlas remains reachable through non-production or test construction while the production `tui::run` path remains legacy until final cutover.
