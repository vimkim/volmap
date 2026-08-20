# Backlog (post-v1, from SPEC "Backlog")

- **B1 TUI slot navigation + interpretation view.** TUI currently stops at
  pages and never enriches (`tui.rs:66`, placeholder tabs `tui.rs:755`).
  Needs slot navigation + an enrichment path first; render the same
  projections as ticket 05. Slots were never in the terminal-parity contract,
  so this is an extension, not a parity debt.
- **B2 Exported-HTML slot UI.** Facts are already frozen in exports after
  ticket 05; the embedded JS (`export.rs:246`) has no slot rendering at all.
- **B3 REC_BIGONE reassembly + interpretation.** Walk
  OVERFLOW_FIRST_PART/REST_PART (research §2.4/§6.2 step 3), reassemble, then
  interpret with the ticket-02 decoder. Mind ADR-0001: reassembled values are
  disclosure-eligible only as decoded typed values, not raw bytes.
- **B4 Type-coverage completions.** SET/MULTISET/SEQUENCE element decoding
  (research §"could not be verified" — element layout unstudied), ENUM
  literal lookup from the domain enumeration substructure, BIT/VARBIT
  rendering policy under no-hex disclosure, codeset-aware transcoding,
  root-class heap records via native-endian `boot_dbparm` (per-ABI work).
- **B5 Composed enrichments advance the revision more than once.**
  Selecting a slot and then interpreting it can publish three revisions
  (structural page, relocation edge, interpretation), and only the final view
  is stored in the live session, so the intermediate revision numbers are
  unreachable. Nothing requests them — every client follows the returned
  `result_revision` — and the CLI has always composed enrichments this way, so
  this is a tidiness debt rather than a defect. Fixing it means either storing
  each intermediate view or making one enrichment call publish once.
- **B6 TDE-opaque interpretation is tested only at the graph layer.** The
  panel's degradation path is covered for a NULL-class heap
  (`web::tests::a_page_that_cannot_be_interpreted_states_its_reason_in_the_panel`)
  but not for an encrypted page, which needs a TDE fixture wired into the web
  test state.
