# 03: Project typed byte coordinates

**What to build:** Add the Rust-owned typed stored-attribute extent, metadata anchors, and coordinate projections required by W2 in the [implementation specification](../implementation-spec.md) and ADR-0005. This ticket does not add UI interaction.

**Blocked by:** 01: Freeze frontend and release evidence.

**Status:** ready-for-agent

- [ ] Closed numeric `ByteExtent`, `BytePoint`, `MetadataAnchor`, selection identity, and page-geometry disposition types replace browser-facing decimal-string arithmetic for this feature.
- [ ] Constructors check overflow, origin, `end_exclusive == start + length`, record containment, page-content containment, physical-page prefix, and volume-file offset.
- [ ] The enclosing record-relative attribute extent is authoritative and the body-relative duplicate `Withheld.offset/length` is normalized or removed without weakening byte withholding.
- [ ] Fixed and fixed-NULL attributes project the stored extent and exact bound-bit byte/bit anchor.
- [ ] Variable attributes project stored extent plus both responsible offset-table entry extents; variable NULL projects an exact zero-width point.
- [ ] OOS projects the complete proven inline attribute extent and relationship without claiming logical payload placement or inventing an OOS prefix width.
- [ ] A relocation source reports target-record coordinates but no source-page/file attribute extent; target page coordinates appear only with loaded, matching target slot geometry.
- [ ] Projection tests pin all four coordinate systems for ordinary, NULL, OOS, withheld, malformed, and relocation source/target fixtures and enforce disclosure sentinels.
