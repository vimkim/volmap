# 05: Harden the terminal session

**What to build:** Complete the focused session, renderer, and terminal-host reliability needed for production use while retaining the three-mode product boundary. Follow the resource, scheduling, text-safety, and compatibility contract in the [focused TUI implementation specification](../implementation-spec.md).

**Blocked by:** [04: Interpret the selected record](04-interpret-selected-record.md).

**Status:** ready-for-agent

- [ ] The terminal host provides correct TTY validation and RAII cleanup for raw mode, alternate screen, mouse capture, and cursor visibility on normal exit and every error path.
- [ ] ANSI/Unicode and monochrome/ASCII profiles preserve all semantic distinctions; hostile controls, wide glyphs, combining sequences, clipping, and ellipsis cannot corrupt geometry or emit terminal controls.
- [ ] Renderer-produced, generation-bound hit regions are clipped and non-overlapping, and every mouse action has a keyboard-equivalent semantic action.
- [ ] Resize preserves mode, exact revision, identities, selected record, interpretation state, and semantic anchors; 59×19 shows a reversible too-small scene and 60×20 restores the same state.
- [ ] Input wins over a simultaneously ready worker completion; resize may coalesce, input is never dropped, stale completions and stale clicks are ignored, and idle polling does not redraw.
- [ ] Retained state is bounded to the current immutable view, visible Sector rows plus overscan, one Page distribution, one interpretation, two terminal frames, and one pending completion.
- [ ] Scripted-host and PTY tests cover keyboard/mouse equivalence, resize recovery, worker ordering, cancellation, failure/non-adoption, quit, and terminal cleanup.
- [ ] The focused twelve-golden matrix, renderer properties, resource assertions, and all relevant merge-base adapter tests pass without adding Ratatui or a generic widget/state framework.
