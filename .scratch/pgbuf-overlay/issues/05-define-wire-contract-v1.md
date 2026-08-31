Type: grilling
Status: resolved
Blocked by: 03

# Define wire contract v1

## Question

Specify the versioned semantic wire contract for the state-only first phase, honoring the design reference's rules (semantic fields only; never serialize `PGBUF_BCB` or `FILEIO_PAGE`; versioned; bounded):

1. Per-page record fields — residency, latch mode, waiter present, fix count, dirty, flushing-to-disk, async-flush-requested, to-vacuum, LRU zone, page LSA, oldest-unflush LSA, physical page type. Which of these are v1 and which wait.
2. Payload shape — a bulk resident-set scan returns only resident pages, so `NOT_RESIDENT` is expressed by omission and the payload is pool-sized, not volume-sized. Decide whether a point `InspectPage(VPID)` also exists in v1 or only the bulk scan.
3. Capture semantics — a capture token bound to database identity and server incarnation; per-scan monotonic sequence; explicit statement that a pool traversal is not an atomic snapshot.
4. Limits — page/time/byte caps, cancellation, backpressure, and the refusal shape when the parameter is off or the caller is not permitted.
5. Encoding and versioning — field naming, version negotiation on handshake, additive-evolution rules (volmap's projections already follow additive-since-version-1).

The design reference's consistency classifications (`SYNCHRONIZED_WITH_MAIN_VOLUME` and friends) stay reserved for the later digest phase; v1 must not fake them from state bits alone.

## Comments

2026-08-26, premise from ticket 03: the channel is an AF_UNIX `SOCK_STREAM` socket speaking versioned JSON-lines — a JSON handshake (version, database identity, server incarnation) followed by newline-delimited JSON records. This ticket defines the handshake and record schemas on that framing.

2026-08-21, hard constraint from ticket 01 ([Branch exposure parity](../research/branch-exposure-parity.md)): the wire contract must never carry raw `ptype` values — OOS inserts `PAGE_OOS` mid-enum (`storage_common.h:159`), shifting raw values ≥ 8 by +1 between branches. The producer maps `PAGE_TYPE` to a wire-owned semantic page-kind vocabulary via a per-branch table, including an `oos` kind that develop never emits.

## Answer

Resolved with the user, 2026-08-26. Wire contract v1, on the ticket-03
framing (AF_UNIX `SOCK_STREAM`, versioned JSON-lines):

- **Record fields (v1, all from the one lock-free scan):** page identity
  (`volid`, `pageid`); `latch_mode` (`none|read|write|flush`),
  `waiter_present`, `fix_count` — the three decoded from one coherent atomic
  load; `dirty`; `flushing`; `async_flush_requested`; `to_vacuum`;
  `lru_zone` (`lru1|lru2|lru3|void|invalid`); `page_lsa`;
  `oldest_unflush_lsa`; `page_kind` (semantic vocabulary, never raw
  `ptype`). The LRU *list index* is excluded as engine-internal quota detail.
  All fields are optional on read for additive evolution.
- **Operations: bulk resident-set scan only.** `NOT_RESIDENT` is expressed by
  omission, so payloads are pool-sized. No point `InspectPage(VPID)` in v1 —
  volmap's page view joins against the latest cached scan so staleness stays
  uniform. This is an explicit, recorded deviation from the design
  reference's point-first delivery order: the reference was written
  digest-first, while v1 is state-only and the heatmap needs the whole pool.
  The point op returns in the digest phase, where per-page capture
  bracketing belongs.
- **Capture semantics:** the handshake carries protocol version, database
  identity, and a server-incarnation id; on incarnation change the client
  drops everything. Each scan is bracketed: a header line (monotonic
  `scan_seq` per incarnation, start time), the record lines, a footer line
  (end time, record count, truncation flag). No per-record timestamps. The
  spec states plainly that a pool traversal is not an atomic snapshot.
- **Conventions:** snake_case field names; enums as lowercase strings;
  additive-only evolution within a major version; unknown fields ignored on
  both sides.
- **Limits and refusals:** at most 2 concurrent clients (volmap + one manual
  debug session); a server-side minimum scan interval of 100 ms; bounded
  write buffer with disconnect-on-stall so the inspector daemon never
  blocks; refusals are JSON error objects with stable lowercase codes:
  `parameter-off`, `version-unsupported`, `busy`, `rate-limited`,
  `incarnation-changed`.
