# 02: Observe the selected page securely

**What to build:** An operator can request a selected Page's authenticated, identity-bound buffer observation and see its state and limitations beside observed disk state. The complete path uses a real bounded socket decoder, broker, normalized HTTP response and selected-Page presentation, with deterministic producer fixtures.

**Blocked by:** 01 — Configure optional runtime attachment safely; external CBRD-27398 delivery of the agreed versioned wire contract and revision/hash-pinned canonical conformance corpus. A live engine is not required for this slice; inventing a substitute canonical protocol does not satisfy the external gate.

**Status:** complete

- [x] Vendor an exact revision/hash-pinned producer-owned corpus and verify it offline. Resolve semantic conflicts explicitly with the producer owner rather than adding browser exceptions. CUBRID producer implementation remains external.
- [x] Authenticate Linux AF_UNIX SOCK_STREAM peers with SO_PEERCRED and exact effective-UID equality, with no root/group exception. Check private-directory and socket ownership before trusting observations. Verify database-creation identity and the complete permanent-volume set of volid, volume-creation identity, device and inode, excluding temporary volumes. Names or paths alone and subset identity proof are insufficient; oversized identity proof refuses attachment.
- [x] Bind observations to the producer's unpredictable incarnation and negotiated protocol. The handshake supplies shared/private LRU topology counts. Identity mismatch produces refused with stable identity-mismatch; refusal or protocol incompatibility clears evidence and requires explicit retry. Sanitize verification, protocol, database fingerprint and abbreviated incarnation; never disclose raw peer identity or socket details.
- [x] Decode versioned JSON-lines with snake_case fields and lowercase semantic enums. Ignore unknown optional same-major fields within limits; reject missing mandatory identity, framing or count fields. Consume semantic page kind rather than native ordinals, including branch-dependent mappings, and never expose packed flags.
- [x] Present VPID, semantic page kind, latch mode none/read/write/flush, waiter presence, fix count, dirty, flushing, async_flush_requested, to_vacuum, page LSA and oldest-unflush LSA. Preserve the coherent latch-word tuple and coherent LRU flags tuple without claiming record-wide or pool-wide atomicity. Present LRU zone lru1/lru2/lru3/void/invalid, list kind shared/private/none/invalid and exact kind-local index or null when not applicable; indices are incarnation-local.
- [x] Validate scan sequence/start header, records and explicit end/count/truncation footer, including raw counts before deduplication and increasing incarnation-local sequence. Sequence is not an event sequence; use capture intervals, not invented per-page timestamps. Duplicate VPIDs are ambiguous/unknown, never last-write-wins.
- [x] An evaluated requested Page omitted from a complete scan is observed not-resident, not guaranteed currently absent. Partial omissions and unevaluated Pages are unknown. Valid truncated scans publish partial evidence; malformed streams, missing footers or assembly overflow discard the whole unfinished assembly and retain only prior usable, unexpired evidence with its original age. Never synthesize a footer or merge scans into apparent completeness.
- [x] Enforce binary-byte limits before unbounded allocation: 65,536 records, 64 MiB framed scan, 4 KiB record/control frame including newline, 64 KiB handshake and nesting depth 16; connect plus handshake within 500 ms and whole scan exchange including drain/footer within two seconds. Parse incrementally without buffering the whole wire stream.
- [x] Account at most 48 MiB per decoded scan and 128 MiB total broker memory, including allocated capacities, indexes, duplicate tracking, parser buffers, retained/in-flight captures, response-held older captures and serialization. No spill or history; aggregate admission can stop below an individual object limit. Replacement does not uncharge still-referenced memory. RSS is a separate measurement.
- [x] POST /api/v1/runtime/page-buffer/observe returns Cache-Control: no-store and echoes accepted ordered VPIDs and scope/request epoch, normalized states, capture identity/interval, limitations, producer completeness and separate requested/evaluated counts. Enforce at most 512 VPIDs, 64 KiB request body and 1 MiB response, eight admitted requests including waiters with no extra queue, a 2.5-second deadline and HTTP 429 overload. A requested scope never changes the producer's bulk-scan cap.
- [x] Maintain one connection, one latest capture and at most one in-flight scan per session from the first working attachment, with a 500 ms scan-start floor. This slice supports explicit selected-Page refresh; automated lifecycle and viewport scheduling follow in tickets 03 and 04 without temporarily removing safety limits.
- [x] From the first observation, enforce monotonic conservative age from broker scan-request start through serialization, adding browser full HTTP round trip and elapsed time. Cache reads never renew capture age; source wall time is display-only. Fresh means known upper age within two caller intervals; uncertain evidence is not fresh. Evict browser and broker evidence after 30 seconds even while paused. Revoke incompatible late adoption on route, scope, generation, overlay, pause or incarnation changes, checking echoed scope/epoch independently of cancellation.
- [x] Selected-Page detail and an initial residency mark distinguish observed residency, observed nonresidency, partial/ambiguous unknown, unavailable and expired evidence with non-color labels. Storage facts remain intact; nothing claims page-image correspondence, commit visibility, durability or event causality.
- [x] Test the production decoder using scripted socket chunking/coalescing, EOF, stalls, malformed framing, additive fields, count/sequence/identity errors, duplicate ambiguity and valid partial scans. Exercise the real HTTP and browser boundaries, plus deterministic highest-interface broker tests and actual I/O timeout checks. Cover below/at/above applicable caps, including retained plus in-flight plus response-held memory; report named executed cases and nonzero counts. Peer-credential isolation requires a suitable environment, not a silent skip.


## Comments

2026-09-10 — Implemented the real producer-to-selected-page path. The external
contract gate is satisfied by producer commit
`f8c068f771ccd141a3c2f08541fe75c84e41ce53` and its verbatim revision-2 corpus:
106 checksummed files and 39 exchange cases. Producer implementation remains
external; live-engine integration is ticket 07.

The broker authenticates real Linux peers, verifies complete permanent-volume
identity, bounds incremental decoding and retained memory, coalesces eight
admitted callers, and preserves age and sequence across failed refreshes.
The HTTP and browser paths expose normalized selected-page evidence alongside
disk facts, with 500 ms caller freshness, late-result guards and 30-second expiry.

`just verify` passed: 293 Rust tests, 55 frontend tests and 7 browser cases
(3 existing Rust ignores and 1 existing Firefox skip). Both new Chromium and
Firefox producer journeys passed. Independent Standards and Spec re-reviews
reported zero unresolved substantive findings after regression-backed fixes.
See [the verification record](../verification/02-selected-page.md) for named
cases, resource-bound evidence, full gate log and screenshots. Later lifecycle,
viewport and release-readiness work remains in tickets 03–08.
