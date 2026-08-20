# Explicit-target disclosure of decoded attribute values

The original disclosure rule was absolute: "Version one never displays application
payloads" (`CONTEXT.md`, Application payload). It existed to keep Volmap Inspector a
structure inspector rather than a data-extraction tool — an offline reader of raw
volumes must not become a way to dump user data wholesale. Record interpretation
(decode layout in `docs/record-interpretation-research.md` §4; working decode proven
on branch `prototype/record-interpretation`) makes decoded attribute values the
whole point of the feature, so the absolute rule had to be amended rather than
silently violated.

**Decision:** replace the absolute ban with *explicit-target disclosure*. Decoded,
typed attribute values are **retained** in the inspection graph **and displayed** —
but only for records the operator explicitly deep-inspected. The rule gates both
retention and display: nothing decodes as a side effect of fast inspection or of
enriching an unrelated target, so wholesale disclosure stays impossible. Raw or
undecodable payload bytes remain withheld everywhere: a type outside the supported
decode set renders as a typed placeholder (type name, offset, length, reason) with
**no hex fallback and no raw bytes**.

## Consequences

- Exports carry decoded values for inspected records. An HTML inspection export
  freezes one revision, and interpretations committed to that revision are part of
  its facts; sharing an export now shares the values the operator chose to inspect.
- OOS chain enrichment is unchanged: it validates structure without retaining
  payload bytes. The test pinning this,
  `oos_enrichment_validates_a_chain_and_retains_no_payload_bytes`
  (`tests/inspection_scan.rs`), stays valid and untouched — an OOS column in an
  interpreted record renders as an entity reference to the chain, never inline
  content.
- Adapters still never decode: interpretation is graph evidence produced by
  deep-inspection enrichment, and every adapter projects the same committed facts.
