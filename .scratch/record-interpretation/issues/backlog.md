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
