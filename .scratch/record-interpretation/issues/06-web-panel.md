# 06 — web: enrichment arm + slot-panel rendering (feature first light)

Blocked by: 05
Blocks: — (v1 complete)

## Goal

Clicking a record in the web page view shows its interpretation in the slot
detail panel (D6). This ticket closes the v1 loop.

## Work

- `parse_enrichment_target` (`web.rs:1381`): new `record:v:p:s` selector →
  `enrich_record_page` (page granularity, D8). Keep `slot:` behavior for
  structural enrichment; close the known gap: web slot-selector enrichment
  must also follow relocation edges the way the CLI does (`web.rs:1286` only
  calls `enrich_page` today vs `cli.rs:570`).
- `SlotResourceProjection` (`web.rs:1181`): add interpretation +
  class-representation fields; `slot()` handler surfaces them when published,
  404-equivalent absence stays as-is when not yet enriched.
- `showSlot` (`web.rs:1706`) + `#slotDetail` panel: render class/schema
  header (class name, reprid), then attribute rows name = value; NULL and
  unresolved states rendered like `classNameLabel` (`web.rs:1683`)
  `unresolved (reason)`; OOS stubs render as links into the existing OOS
  view/validate flow; an "Interpret records" affordance triggers the
  enrichment POST (mirror `enrichOos`, `web.rs:1709`).
- Relocation records: show forward OID + interpreted target per D1.

## Acceptance

- End-to-end test (existing web test style): serve demodb fixture → POST
  `record:` enrichment → GET slot → interpreted values in projection; UI
  renders values, NULLs, unresolved reasons, OOS link.
- A TDE-opaque or NULL-class page shows the degradation reason in the panel,
  structural facts intact, no error page.
- No bytes/hex anywhere in the rendering (D12).
