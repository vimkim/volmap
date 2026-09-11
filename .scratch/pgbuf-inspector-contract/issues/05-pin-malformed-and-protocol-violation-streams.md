# 05: Pin malformed and protocol-violation streams

**What to build:** A Volmap implementer can exercise the whole-assembly discard rule and broken-connection handling on real bytes for every malformed condition the decisions name, using a fixed discard-reason vocabulary. Every malformed stream in the corpus is reproducible from a canonical stream plus a documented mutation, so the corpus never contains unexplained bytes.

**Blocked by:** 03 — Pin complete scans with the full semantic vocabulary. Independent of 04 in content; both regenerate the shared checksum file and manifest, so whichever lands second regenerates them again.

**Status:** ready-for-agent

- [ ] The corpus README publishes the discard-reason vocabulary: `missing-footer`, `page-count-mismatch`, `sequence-mismatch`, `incarnation-mismatch`, `invalid-json`, `invalid-utf8`, `frame-too-large`, `depth-exceeded`, `missing-mandatory-field`, `handshake-too-large`, `protocol-violation`, `page-cap-exceeded`, `byte-cap-exceeded`. The contract states that failures of mandatory framing, identity, sequence or count fields discard the whole assembly while a previously published scan stays valid, and that a frame after the footer other than the client's next request is a protocol violation that breaks the connection without invalidating the published scan.
- [ ] Each malformed case records its canonical source stream and the exact mutation applied, in a machine-readable form the test can replay.
- [ ] Cases exist: missing footer at end of stream; `page_count` over and under the emitted frames; footer `scan_seq` differing from the header; header `incarnation` differing from the hello; non-increasing `scan_seq` across two scans; an invalid JSON frame; an invalid UTF-8 frame; a page frame of exactly 4,096 bytes accepted beside one of 4,097 bytes discarded, padded through an additive field; nesting depth 16 accepted beside depth 17 discarded; a page frame missing `volid` and another missing `pageid`; a page frame after the footer (`connection-closed`, published scan retained).
- [ ] Tests prove that replaying each documented mutation on its canonical source reproduces the stored malformed bytes exactly, that every expected-outcome file parses and uses only the published outcome and discard-reason vocabularies, and corpus integrity after regeneration with the revision incremented.
- [ ] Execution evidence with named cases and nonzero counts is recorded in this ticket's comments with the exact commit.
