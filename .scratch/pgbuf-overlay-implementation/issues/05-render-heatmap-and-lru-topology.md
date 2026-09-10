# 05: Render the heatmap and LRU topology

**What to build:** An operator can read sampled buffer state over the Volume/Sector heatmap, switch to LRU topology, and inspect exact selected-Page membership without confusing runtime evidence with storage allocation or event history.

**Blocked by:** 04 — Observe visible pages with shared, bounded demand.

**Status:** complete

- [x] Preserve existing storage colors beneath cyan residency outlines, amber dirty corners and static magenta flushing edges. Use the accepted visual prototype as design evidence, not production code or proof of performance.
- [x] Provide an alternative LRU-topology color mode rather than mixing LRU colors into storage allocation colors. Consume normalized shared/private topology counts and coherent zone/list-kind/kind-local-index observations; show exact indices in selected-Page detail, including none/invalid/null meanings and incarnation-local scope.
- [x] Derive only observed per-list summaries from validated capture records and explicitly label them partial when the source is partial. Do not present native counters/quotas or combine captures into an apparent pool snapshot.
- [x] Keep latch/waiter/fix and other detailed state in selected-Page detail rather than dense-grid animation. Retain semantic page kind, dirty/flushing/async-flush-requested/to-vacuum and log-position detail without inventing transition start/end, duration, counts, cause or durability.
- [x] At both Volume and Sector scales, distinguish capability, freshness, pause, expiry and observation coverage independently. Source-unavailable, unevaluated/partial unknown, duplicate ambiguity and observed nonresidency must not share a misleading absence mark. Prefer no overlay to fabricated evidence.
- [x] Provide keyboard-operable controls, stable compatible focus and non-color glyphs/text/accessible names for runtime states. Respect reduced motion and high contrast; polling and age ticks do not create repeated announcements. Initial semantics must be accessible before the final gate in ticket 06.
- [x] Test the complete normalized HTTP-to-render path with real frontend state/effects and Chromium/Firefox semantic assertions at both scales. Cover state-mark/LRU switching, exact membership, partial summaries, identity changes, unknowns and expired/unavailable evidence; use a small screenshot set only as supporting visual evidence.
- [x] Runtime rendering never mutates inspection facts, revisions, generations, diagnostics, outcomes, export content or TUI behavior. No AOUT history, protected resident-page inspection, image correspondence or unrelated kernel-cache/attribute-selection feature is introduced.


## Comments

2026-09-10 — Implemented shared Volume/Sector state marks and the explicit LRU
color mode, capture-bound topology counts and per-list summaries, normalized
membership validation, exact selected-Page detail and non-color lifecycle
semantics. The current-batch disclosure table includes zone/list kind so Volume
LRU evidence remains accessible without navigating into a different capture.

`just verify` passed: 306 Rust tests, 73 frontend tests and 31 browser cases;
three pre-existing Rust ignores and one existing Firefox skip remain. All 14 new
rendering cases ran in Chromium and Firefox. Independent Standards and Spec
reviews have zero unresolved findings. The final static-musl build/ELF check also
passed after the review fix. See [the verification record](../verification/05-heatmap-and-lru.md)
and [operator guidance](../../../docs/runtime-overlay.md).

Density/accessibility acceptance, real-engine producer verification and release
readiness remain tickets 06–08; this ticket does not claim those gates.
