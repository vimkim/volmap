# 04: Add attribute selection and cross-highlighting

**What to build:** Render the typed W2 facts as synchronized attribute table, record map, page-content map, and coordinate detail interactions. Follow W3 in the [implementation specification](../implementation-spec.md).

**Blocked by:** 02: Establish the React compatibility viewer; 03: Project typed byte coordinates.

**Status:** ready-for-agent

- [ ] Hover and keyboard focus preview an attribute; click, Enter, and Space commit; Escape clears without changing the URL or browser history.
- [ ] Committed identity is record OID + representation id + attribute position, never display name or array index.
- [ ] A same-identity refresh re-resolves the current projected extent; a record/representation/position mismatch clears selection with an explicit reason.
- [ ] Record-relative, page-content, physical-page, and volume-file coordinates render from Rust-projected facts only.
- [ ] Fixed NULL retains its byte band and bound-bit anchor; variable NULL renders a point/caret and both offset anchors.
- [ ] OOS highlights only proven inline storage and exposes the semantic link; withheld values use the enclosing proven extent without revealing bytes.
- [ ] Relocation target record geometry never appears on the source page; loading the target slot enables target page/file highlighting.
- [ ] Chromium and Firefox cover pointer/keyboard equivalence, refresh preservation/clearing, all storage cases, focus visibility, reduced motion, sparse announcements, and disclosure.
