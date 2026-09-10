# Ticket 05 — Heatmap and LRU topology verification

Date: 2026-09-10. Baseline: `d41c772`. The consumer revision is the commit
containing this record. Protocol, producer corpus and storage contracts are
unchanged.

## Delivered behavior

Volume and Sector share static cyan residency outlines, amber dirty corners and
magenta flushing edges. State mode preserves storage colors and finding outlines.
The explicitly selected LRU mode replaces cell backgrounds with zone colors and
neutral unknown/nonresident cells. Private membership uses a dashed inset.
Glyphs, accessible Sector summaries, per-page accessible labels and the bounded
observation table supply non-color semantics, including zone/list-kind data for
the currently retained Volume capture. Latch/waiter/fix, semantic page kind,
additional flags, log positions and exact kind-local membership remain available
in selected-Page detail. See [operator guidance](../../../docs/runtime-overlay.md).

Topology counts are decoded from capture metadata; normalized zone/kind/index
combinations and index bounds are validated before rendering. Per-list summaries
use only resident rows of one current HTTP batch. They explicitly disclose partial
producer scans and scope-limited counts even when the source scan is complete.
They do not accumulate captures or claim native counters or quotas.

Capability, conservative age and caller cadence, pause, coverage and expiry remain
independent. Source failures hide map marks without relabeling retained capture
detail as new evidence. Expiry and identity revocation clear the batch, including
its list summary. Runtime controls modify only observation presentation; storage
projections, inspection revision/generation, outcomes, exports and TUI behavior
are unchanged.

## Executed tests

| Requirement | Evidence |
| --- | --- |
| State marks and LRU switching at both scales | `volume/0` and `sector/0/0 composes state marks and switches to explicitly labelled LRU colors`: real authenticated scripted producer, actual normalized HTTP decoder, state/effects and browser; keyboard selector operation; selected retained row and topology counts; current-capture table exposes zone/list kind |
| Storage colors and static channels | Both-scale `separates partial unknowns, duplicate ambiguity, nonresidency and expired evidence`: computed storage background unchanged in state mode; dirty/flushing classes and non-color labels/glyphs; private dashed shape in forced colors, reduced-motion mode |
| Coverage and no cross-capture summaries | Same cases use partial omissions, duplicate VPID, unevaluated rows, two lists, then an independent complete capture with omissions; old list summary disappears rather than accumulating |
| Pause, stale age and expiry | Same cases freeze adoption, advance the browser clock beyond freshness and retention, assert stale text and expiry, then resume into observed nonresidency |
| Exact membership and invalid index rejection | `selected detail exposes exact membership and null meanings; inconsistent topology is refused`: private index 1, none/invalid with null index; private index equal to count rejected |
| Source/identity independence at both scales | Both-scale `source failure hides marks and paused incarnation change revokes every list`: capability metadata changes identity while paused, clears marks and topology, requires explicit retry; subsequent HTTP failure removes marks while disk navigation remains usable |
| Compatible focus and quiet updates | Sector test retains focused page through polling and moves to adjacent page with ArrowRight. New observation controls, glyphs, legend, capture-age and table updates introduce no live region. Existing polling/lifecycle browser coverage remains active |
| Inspection independence | Both-scale scenarios assert the original snapshot/revision label after runtime changes. Changes are confined to the frontend observation model/decoder/presentation; no Rust inspection/export/TUI code changes |

The new browser scenarios run in **both Chromium and Firefox**. The baseline
producer path remains a real socket → Rust broker → HTTP → React route. Adversarial
presentation scenarios transform the real normalized HTTP response while retaining
request echoes and capture metadata, and execute the production browser decoder,
model and effects. These are deterministic consumer tests, not real-engine evidence.

Red/green evidence: the initial mode-switch test failed on the absent selector;
the invalid-index test failed because the index was accepted; the review regression
failed because the Volume table lacked a zone cell. Each passed after its fix.

Final `just verify`: **passed (exit 0)**. **306 Rust tests, 73 frontend tests
and 31 browser cases passed**, with three pre-existing Rust ignores and one
pre-existing Firefox skip. All 14 new rendering cases executed. The gate includes
Rust formatting/Clippy, static-musl ELF, frontend types, reproducible generated
assets, dependency advisories, both browsers and Cargo-only embedding. Full log:
[05-verify.log](05-verify.log). The final generated bundle was also rebuilt into
the release executable after the review fix; [final ELF check](05-final-elf.log)
passed (exit 0).

## Independent review

Standards: zero findings. Spec: one accessibility finding (missing Volume
per-page LRU semantics) fixed by extending the current-batch table and both-browser
assertions. Independent recheck confirmed closure: **zero unresolved findings**.

## Supporting screenshots and limits

- [Chromium Volume](05-volume-chromium.png)
- [Chromium Sector](05-sector-chromium.png)
- [Firefox Volume](05-volume-firefox.png)
- [Firefox Sector](05-sector-firefox.png)

Screenshots support semantic assertions; they do not establish performance or
accessibility conformance. The dense 10,000-page performance/accessibility gate,
real CUBRID producer checks and release-readiness decisions remain tickets 06–08.
No AOUT history, protected resident inspection, image correspondence, kernel-cache
or attribute-selection feature is introduced.
