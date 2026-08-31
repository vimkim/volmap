# 04: Interpret the selected record

**What to build:** Connect `Enter` on a selected live record to the existing Page-granularity record enrichment and show the result as Page-local interpretation detail. Follow F4 and the disclosure rules in the [focused TUI implementation specification](../implementation-spec.md). This is a structural drill-down, not a new persistent route or a raw-record viewer.

**Blocked by:** [03: Inspect Page record distribution](03-inspect-page-record-distribution.md).

**Status:** implemented

- [x] Move the web-private bounded record-selection enrichment recipe below adapters so web and TUI share Page structure, relocation evidence, Page interpretation, and relocation-target interpretation without duplicating policy.
- [x] `Enter` enriches only when the selected interpretation is absent and no durable Page-level failure already answers the request; unsupported, empty, deleted, and non-record regions create no work.
- [x] The interpretation panel shows stable record identity/type, class/table, representation id, relocation origin when present, record-layout regions, and ordered attribute name/domain/state/value projections.
- [x] Existing home/newhome and one-hop relocation behavior is preserved; `REC_BIGONE`, malformed evidence, encrypted-opaque data, root/system heaps, and decode failures render typed limitations or durable reasons.
- [x] Typed decoded values appear only after the explicit selected-record action, and unrequested or undecodable bytes remain withheld under the existing disclosure contract.
- [x] `Esc` closes interpretation and returns to the same record and distribution anchor; ascent, sibling navigation, or quit cancels and deactivates active record work first.
- [x] A completion may adopt only when request, snapshot, base revision, Page, and selected OID still match and the OID re-resolves in the returned view.
- [x] Fixtures cover home/newhome, relocation, unsupported `REC_BIGONE`, tombstones, partial attributes, Page-level failure, cancellation, stale completion, and explicit-target disclosure.
