# 08: Restore selectors, filters, findings, and utility regions

**What to build:** Complete Atlas's non-enrichment interaction parity by restoring the accepted selectors, filters, finding traversal, Page utility regions, overlays, accelerators, and semantic scroll behavior through one deterministic state machine.

**Blocked by:** 07: Open the exhaustive Page workspace.

**Status:** superseded

**Superseded by:** [Volmap focused TUI implementation specification](../../volmap-tui-focused-inspector/implementation-spec.md).

- [ ] `/` and `g` edit the canonical typed Entity selector; valid Volume/Sector/Page targets transactionally construct their canonical trail and invalid input preserves prior navigation state.
- [ ] Normalized filters dim nonmatches without deleting, reordering, disabling, or moving physical topology or focus; direct selection of a nonmatch remains usable and announced.
- [ ] `n`/`N` traverse typed Diagnostic occurrences deterministically and wrap; Page/Slot/OOS references land on accepted ancestors without parsing subject, message, code, or labels.
- [ ] Facts, Distribution, Slots, Chain, Findings, and Coverage retain independent semantic anchors and the accepted Tab/Shift-Tab, numeric, `d`, `j`/`k`, and mouse-scroll behavior.
- [ ] `?` and `6` expose contextual Help/About through the one modal overlay; editor, overlay, cancellation placeholder, ascent, and root dismissal precedence are deterministic.
- [ ] Existing `[`, `]`, PageUp/PageDown, `f`, `q`, arrows, Enter, Esc, Backspace, and breadcrumb semantics match the accepted state-model contract without browser-like chronological history.
- [ ] Every rendered control has a keyboard path, every pointer path resolves through committed semantic geometry, and paired traces finish with identical state and ordered effects.
- [ ] Seeded arbitrary traces preserve one exact revision, valid ancestry, unique focus, bounded anchors, at most one overlay, deterministic effects, and layout-generation safety.
