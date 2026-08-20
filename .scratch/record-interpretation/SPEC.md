# Spec: Record interpretation in the slot view

Status: agreed 2026-08-21 (grill session, all recommendations accepted).
Primary sources: `docs/record-interpretation-research.md` (layout, cited to
feat-oos `465cf53e3`), branch `prototype/record-interpretation` (working Rust
decode of every demodb table), recon report facts cited as `file:line` below.

## Feature

Clicking a record in a heap slotted page (web view) shows the record's
interpretation: attribute names, domains, and decoded values, resolved via the
record's reprid and the class representation read from the **class object's
own heap record** (never the system catalog — its DISK_REPR is a parallel
statistics structure and its extendible hash is dead code; research §3).

## Decisions (D1–D14)

- **D1 Record types**: `REC_HOME`/`REC_NEWHOME` interpret directly.
  `REC_RELOCATION`: show forward OID *and* the interpreted target (one hop,
  both facts). `REC_BIGONE`: show forward VPID only; interpretation deferred
  (backlog B3). Slot 0 and tombstones keep existing rendering.
- **D2 Class scope**: every class reachable by a valid class OID, including
  system classes. Records in NULL-class heaps (root/boot) degrade with reason
  `root/system heap records are not interpreted` (lifting it requires
  `boot_dbparm`, backlog).
- **D3 Type coverage**: the prototype's proven set decodes in v1: INTEGER,
  SHORT, BIGINT, FLOAT, DOUBLE, DATE, TIME, TIMESTAMP, DATETIME, MONETARY,
  OBJECT, CHAR/VARCHAR/NCHAR/VARNCHAR (incl. LZ4), NUMERIC. Everything else
  (SET/MULTISET/SEQUENCE, ENUM, BIT/VARBIT, BLOB/CLOB, JSON) renders as a
  typed placeholder: type name + offset + length + reason. **No hex, no raw
  bytes — bytes stay withheld everywhere (D12).**
- **D4 OOS columns**: the 16-byte stub renders as an entity reference (OID +
  full length) linking to the existing OOS chain view. No inline reassembly.
- **D5 Interpretation is graph evidence**: produced only by explicit
  deep-inspection targets, stored in the inspection graph, advances the
  revision, freezes into exports. Adapters never decode (CONTEXT.md adapter
  rule).
- **D6 Web surface**: extend the existing slot detail panel (`#slotDetail`,
  `web.rs:1706`); the page view keeps navigating to it.
- **D7 Class representation is a first-class entity**, keyed
  `(class_oid, reprid)`, referenced by interpretations, independently
  renderable (schema panel). The `(volid, sectid) → class` cache is an
  internal index over these entities, not a graph entity. Key must be
  `(volid, sectid)` — heaps span volumes (research §5.3).
- **D8 Granularity**: one click interprets all home records of that page as
  one enrichment (one revision advance, batch publish). Idempotent re-clicks
  do not advance the revision (matches `publish_deep_page` `inspection.rs:4153`).
- **D9 Old representations ship in v1**: reprid ≠ current walks the class
  record's `ORC_REPRESENTATIONS_INDEX` (=2) substructure set (research §3.5).
  Unknown reprid after that walk degrades per D10.
- **D10 Failure surface**: partial interpretation. Per-attribute three-state
  projection (decoded / null / unresolved{reason}) copying the
  `ClassNameProjection` pattern (`projection.rs:222`). Class-level failures
  (no classrepr resolvable, TDE-opaque page, malformed class record) degrade
  the whole panel to the existing structural view + a `DiagnosticRecord`;
  never an error page. Failures publish as durable evidence (mirror
  `page_decode_failure`, `inspection.rs:4131`).
- **D11 Vocabulary**: CONTEXT.md gains **Record interpretation**, **Class
  representation**, and a disclosure term (working name **Explicit-target
  disclosure**); **Application payload** (`CONTEXT.md:179`) and **Deep
  inspection** (`:167`) are amended. Two ADRs (repo's first):
  - ADR-0001: amend the disclosure policy — decoded, typed attribute values
    are retained and displayed only for records the operator explicitly
    deep-inspected; undecodable bytes remain withheld; wholesale disclosure
    stays impossible.
  - ADR-0002: interpretation resolves through the class object's heap record,
    not the system catalog. Consequences recorded: `(volid, sectid)` cache
    key, page-granularity enrichment.
- **D12 Disclosure**: explicit-target disclosure (see ADR-0001). This
  amendment gates retention *and* display; it is why D3's fallback carries no
  hex.
- **D13 Adapters in v1**: web + CLI (`inspect slot:`) + JSONL render the same
  `RecordInterpretationProjection`. TUI (no slot view exists, `tui.rs:755`)
  and exported-HTML slot UI (none exists, `export.rs:246`) are backlog B1/B2;
  their frozen facts are already present in the revision.
- **D14 Process**: tickets under `.scratch/record-interpretation/issues/`,
  worked blockers-first, one fresh `/implement` context per ticket.

## Architecture (recon-verified seams)

New state in `SessionData` (`inspection.rs:986`), following the OID-keyed
precedent of `oos_chains`/`relocation_edges`:

- `class_representations: BTreeMap<(Oid, i32), ClassRepresentationFact>` +
  internal `(VolId, sector) → Oid` index for cache hits.
- `record_interpretations: BTreeMap<Oid, RecordInterpretationFact>`.

Both need: publish fns that clone `SessionData`, bump revision once per
enrichment, refresh a new coverage facet (mirror `refresh_oos_coverage`
`inspection.rs:4605`), re-classify outcome; `verify_unchanged` brackets
(mirror `enrich_page` `inspection.rs:2139`, `:2310`) degrading to
`invalidated_revision()`; idempotency (repeat enrichment returns
`self.clone()`, no bump); `GraphView` accessors (mirror `:3919`/`:3932`) so
HTML export carries the facts.

Byte decoding lives in `src/format/` (new `classrep.rs` + record-value
decoding beside `heap.rs`), `DecodeError` + dotted rule strings for format
violations; human-readable `&'static str` reasons on paths whose text reaches
users verbatim (recon §7 two-vocabulary rule). `HeapRecordEnvelopeFact`
(`format/heap.rs:43`) already supplies reprid/bound-bits/offset-width/body.
Class-name decoding precedent: `decode_class_name` (`inspection.rs:4361`),
charset via `database_charset` (`:4343`).

Web: new `record:v:p:s` arm in `parse_enrichment_target` (`web.rs:1381`);
slot-selector enrichment must also follow relocations (today it only calls
`enrich_page`, `web.rs:1286` — the CLI already does more, `cli.rs:570`).
`SlotResourceProjection` (`web.rs:1181`) and `DataProjection::InspectSlot`
(`projection.rs:110`) both gain the interpretation + classrepr fields.

## Layout facts implementers must not rediscover

All in `docs/record-interpretation-research.md`: reprid = low 24 bits of the
first BE word (§1.2, must use the 24-bit mask); header size from the 3-bit
MVCC flag lookup `[8,16,16,24,16,24,24,32]` (§1.3); feat-oos variable-offset
entries carry flag bits in the low 2 bits — always mask with `!0x3`, OOS bit
`0x1` (§1.4/§7); CHAR and NUMERIC are variable-region types (§4.2 — the
single easiest mistake); varchar prefix/LZ4 rule (§4.4); NUMERIC 3-byte
header (§4.5); class record walk constants (§3.5 + prototype `main.rs`
`parse_class_record`); class records are always 4-byte offset width and can
be REC_RELOCATION (follow, cycle-bounded like `resolve_class_name`
`inspection.rs:2085`).

## Backlog (not v1)

- B1 TUI slot navigation + interpretation view.
- B2 Exported-HTML slot UI.
- B3 REC_BIGONE reassembly + interpretation.
- B4 SET element decoding, ENUM literal lookup, BIT rendering, codeset-aware
  transcoding (v1 assumes UTF-8-compatible display via lossy conversion),
  root-class heap records via `boot_dbparm`.
