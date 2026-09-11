# CUBRID page-buffer inspector wire contract v1 and canonical conformance corpus

Status: ready-for-agent

Scope: CUBRID producer repository work for
[CBRD-27398](http://jira.cubrid.org/browse/CBRD-27398), delivery-plan stage
"Contract consolidation". Consumer: Volmap implementation ticket
[02 — Observe the selected page securely](../pgbuf-overlay-implementation/issues/02-observe-selected-page-securely.md).
Published to the configured local Markdown tracker because the CUBRID
worktree has no configured tracker and JIRA publication is not authorized by
the originating handoff. No implementation, test result, commit, push, JIRA
change or PR is implied by this document.

This specification gives the accepted semantics a byte-exact syntax and a
pinned corpus. It does not reopen any accepted decision; where concrete syntax
required a choice the decisions left open, the choice is listed under
"Concrete-syntax clarifications" in Further Notes. The user accepted all ten
on 2026-09-08, together with the seven implementation tickets under `issues/`.

## Problem Statement

Volmap ticket 02 needs to decode real CUBRID page-buffer observations, but it
is blocked on an external prerequisite it may not satisfy itself: the
producer-owned, versioned wire contract and a revision/hash-pinned canonical
conformance corpus. The accepted decisions fix what the wire means — semantic
page state, one coherent latch-word sample, one coherent LRU flags sample,
bracketed non-atomic scans, identity-bound handshake, stable refusal codes and
hard resource limits — but they do not fix the bytes. Without a byte-exact
contract and an offline oracle, the consumer cannot build a real decoder, the
producer cannot prove its serializer, and the two repositories drift until a
live-engine integration run discovers it.

A live engine is not required for this stage. Inventing a consumer-owned
substitute protocol does not satisfy the gate. The deliverable is an exact,
content-hash-pinned artifact set inside the CUBRID repository that Volmap can
vendor and verify offline, plus proof on the CUBRID side that the canonical
bytes are producible by the producer's own serializer.

## Solution

Deliver, in the CUBRID repository, a versioned contract document for wire v1
and a canonical conformance corpus of JSON-lines streams with expected consumer
outcomes, covering valid complete scans, valid truncated scans, additive
unknown fields, duplicate page identities, every refusal code, handshake
identity cases, malformed and oversize streams, miscounted and mis-sequenced
scans, and limit boundaries. Pin the corpus by a SHA-256 manifest whose
aggregate hash Volmap records together with the producer commit that carried
it. Prove the corpus is producible: an engine-independent, producer-owned
canonical serializer — the module the later bounded BCB scan will feed —
reproduces every canonical stream byte-for-byte in unit tests that are shown
to execute. Both repositories verify the same corpus offline and independently.

The bounded engine producer itself (startup-only parameter, private daemon and
socket, peer credentials, engine identity collection, lock-free BCB traversal)
is the next delivery-plan stage and is out of scope here.

## User Stories

1. As a Volmap implementer, I want a byte-exact contract for wire v1, so that I can build the production decoder against fixed syntax instead of prose semantics.
2. As a Volmap implementer, I want canonical streams for complete scans that exercise every enum value, so that my normalizer is tested against real vocabulary rather than my own guesses.
3. As a Volmap implementer, I want canonical streams for truncated scans with valid footers, so that partial evidence publishes correctly and missing pages stay unknown.
4. As a Volmap implementer, I want streams with additive unknown fields and unknown frame kinds, so that forward compatibility within a major version is proven rather than assumed.
5. As a Volmap implementer, I want streams with duplicate page identities, so that ambiguity handling is pinned and last-write-wins can never creep in.
6. As a Volmap implementer, I want a refusal frame for every stable code, so that each refusal maps to the right runtime capability state.
7. As a Volmap implementer, I want handshake fixtures carrying the complete permanent-volume identity set, so that identity verification, mismatch and oversize refusal are tested offline.
8. As a Volmap implementer, I want malformed streams for missing footers, count and sequence mismatches, incarnation mismatches, invalid JSON, invalid UTF-8, oversize frames and excess nesting depth, so that the whole-assembly discard rule is exercised on real bytes.
9. As a Volmap implementer, I want limit-boundary cases at, below and above every accepted cap, so that enforcement before unbounded allocation is verified.
10. As a Volmap implementer, I want expected outcomes in a machine-readable file per case, so that tests assert against the corpus rather than a human reading of it.
11. As a Volmap implementer, I want the corpus pinned by an aggregate SHA-256 and by the producer commit, so that I can vendor an exact copy and detect drift offline.
12. As a Volmap implementer, I want page-kind names identical to the ones my disk inspection already uses, so that the runtime overlay and disk facts share one vocabulary.
13. As a Volmap implementer, I want log positions expressed semantically as page and offset, so that I never parse a packed engine word from the wire.
14. As a Volmap implementer, I want the rule for absent versus null fields written down, so that "not supplied" and "known empty" are never confused.
15. As a CUBRID producer implementer, I want a producer-owned semantic record type independent of engine layouts, so that the later BCB scan only fills scalars and never serializes engine structures.
16. As a CUBRID producer implementer, I want a canonical encoder whose output is pinned by the corpus, so that the scan, daemon and socket work can start against a proven serializer.
17. As a CUBRID producer implementer, I want the page-kind mapping defined by enumerator name for both supported branches, so that develop never emits `oos` and the format-aligned branch does, without a contract fork.
18. As a CUBRID producer implementer, I want defined behavior for every internal value with no semantic mapping, so that an unexpected engine state can never leak a raw ordinal.
19. As a CUBRID producer implementer, I want the footer reservation bound published, so that a valid footer always fits after the last admitted page.
20. As a CUBRID producer implementer, I want frame limits enforced by the encoder, so that a record that cannot fit is refused before it is written.
21. As a CUBRID reviewer, I want the contract to cite the accepted decisions rather than restate or reopen them, so that review checks syntax against settled semantics.
22. As a CUBRID reviewer, I want no new runtime or build dependency and no engine behavior change in this stage, so that the change is a documentation-and-test addition.
23. As a CUBRID reviewer, I want unit tests proven to have executed with named cases and nonzero counts, so that a successful build or an empty test run is never reported as passing.
24. As a CUBRID reviewer, I want the corpus verifiable with standard tooling, so that a reviewer can check integrity without project-specific scripts.
25. As a CUBRID reviewer, I want the contract to state plainly that a scan is not an atomic snapshot and that omission means non-residency only in complete scans, so that no reader over-trusts the data.
26. As a cross-repo integration owner, I want the corpus content hash identical on the format-aligned iteration branch and the develop port, so that the hash is the authority and the commit is provenance.
27. As a cross-repo integration owner, I want the vendoring procedure written down, so that the evidence manifest can record corpus hash and producer commit unambiguously.
28. As a cross-repo integration owner, I want a corpus change procedure that bumps the revision and requires matching evidence on both sides, so that a silent corpus edit cannot pass.
29. As a maintainer of either repository, I want a versioning rule for additive and breaking changes, so that a vendored corpus never becomes ambiguous about which contract it pins.
30. As a maintainer of either repository, I want large boundary cases described by a deterministic recipe with a pinned hash, so that the corpus stays small enough to vendor while still pinning the bytes.
31. As a maintainer of either repository, I want time fields declared as display metadata, so that no one derives freshness from producer wall clocks.
32. As an operator, I want the contract to forbid pointers, raw flag words, raw page-type ordinals, thread identities and page contents on the wire, so that enabling observation cannot disclose engine internals or application data.
33. As an operator, I want the consumer to treat unrecognized enum strings as unknown state rather than errors, so that a newer producer does not break an older viewer.
34. As a release reviewer, I want this stage to claim only contract, corpus and serializer evidence, so that nobody mistakes it for engine integration.

## Implementation Decisions

### Scope, ownership and placement

1. This stage delivers three producer-owned artifacts in the CUBRID
   repository: the wire v1 contract document, the canonical conformance
   corpus with its manifest, and an engine-independent canonical serializer
   with unit tests. Landing points follow the delivery plan's assignments (a
   versioned producer-owned documentation directory in CUBRID and a vendored
   fixtures directory in Volmap); the plan owns exact paths and may move them
   together with manifest references.
2. Work happens in a fresh dedicated worktree and branch forked from the
   accepted format-aligned baseline `e1e651debf6cc100172bde96603b17424f9c135a`,
   per the branch decision. The develop port cherry-picks the same artifacts.
   The corpus content is branch-independent by construction, so the aggregate
   hash must be identical on both branches; a differing hash is a defect.
   The analysis worktree's current HEAD is not an implementation base.
3. No engine behavior changes in this stage: no system parameter, daemon,
   socket, peer-credential check, identity collection, BCB traversal or
   external shell testcase. No new runtime JSON dependency, no inspector
   compile option, no Windows or non-server exposure. Preserve CUBRID error
   handling, indentation, include ordering and license headers.
4. The contract document is normative for syntax only. Semantics remain in
   the accepted decision tickets: transport (03), gating (04), wire v1 (05),
   security (08), verification (11), exact LRU membership (14) and budgets
   (15). The document cites them and states where concrete syntax made a
   choice they left open; it never weakens them.

### Framing and canonical form

5. Transport is the accepted AF_UNIX `SOCK_STREAM` channel. Every frame is one
   JSON object encoded in UTF-8 followed by exactly one line feed. Frames are
   objects only. No frame contains a raw line feed inside its JSON text.
6. Frame kind is the string field `type`, always the first key. Kinds are
   `client_hello`, `server_hello`, `error`, `scan_request`, `scan_header`,
   `page` and `scan_footer`. Field names are snake_case. Enum values are
   lowercase strings; multi-word values use hyphens, matching the accepted
   refusal codes and Volmap's existing page-kind names.
7. Canonical producer form, which the corpus pins byte-for-byte: keys in the
   documented order; no whitespace outside strings; integers in base 10 with
   no leading zeros, sign only when negative, no exponent or fraction;
   `true`, `false` and `null` literals; strings with minimal JSON escaping.
   All producer-emitted strings are ASCII by construction. The canonical form
   binds the producer. Consumers accept any RFC 8259 form within limits and
   are never required to be byte-strict.
8. Accepted limits apply as binary bytes including the line feed: control and
   page frames at most 4,096 bytes; the server hello at most 65,536 bytes;
   JSON nesting depth at most 16; a whole scan at most 64 MiB; at most 65,536
   visited slots and 65,536 emitted pages per scan. The producer enforces
   these at encode time and never emits a frame that violates them. The
   consumer enforces them before allocation. Depth counts nested objects and
   arrays from the frame object as depth 1.
9. Absent versus null: a field the contract marks optional may be absent,
   meaning "not supplied by this producer version". A field the contract
   marks nullable may carry `null`, meaning "known to be empty or not
   applicable". Mandatory framing, identity, sequence and count fields are
   never optional; their absence discards the whole assembly.
10. Forward compatibility within a major version: consumers ignore unknown
    fields and unknown frame kinds that appear between a scan header and its
    footer, counting their bytes against limits but not against page count.
    Consumers treat an unrecognized string in any enum field as an unknown
    state and keep the page frame valid. Producers add fields or frame kinds
    only with a minor version increment; incompatible changes require a new
    major version and a new contract directory.

### Exchange order

11. On connection the server authenticates the peer before reading any byte,
    per the security decision; that check is stage-two work, but the
    contract states that an unauthorized peer is closed without any frame.
12. The client sends one `client_hello`. The server replies with exactly one
    `server_hello` or one `error`, then either continues or closes as the
    error code dictates. The client then sends `scan_request` frames one at
    a time. Each request yields either one `scan_header`, zero or more `page`
    frames and one `scan_footer`, or exactly one `error`. A second request
    before the footer or error is a protocol violation; the server closes
    without a frame.
13. No frame is ever emitted between a header and its footer other than
    `page` frames and future additive frame kinds. A scan that stops early
    for any accepted reason still ends with a valid footer marked truncated.
    Abnormal termination is a closed connection with no footer; the consumer
    discards the unfinished assembly and retains only prior usable,
    unexpired evidence. A stalled writer is disconnected per the budgets.
14. Any frame received after a footer other than the client's next
    `scan_request` is a protocol violation. The already-published scan stays
    valid; the connection is treated as broken.

### Frame schemas

Field tables list name, JSON type, whether the field is mandatory (M),
optional (O) or nullable (N), and meaning. Order is canonical order.

`client_hello` (client to server, control-frame limit):

| Field | Type | M/O/N | Meaning |
| --- | --- | --- | --- |
| `type` | `"client_hello"` | M | Frame kind |
| `supported_majors` | non-empty array of positive integers | M | Protocol majors the client can speak |
| `expected_incarnation` | string | O | If present and different from the server's incarnation, the server refuses with `incarnation-changed` |
| `client` | ASCII string, at most 64 bytes | O | Informational client label, never interpreted |

`server_hello` (server to client, handshake limit):

| Field | Type | M/O/N | Meaning |
| --- | --- | --- | --- |
| `type` | `"server_hello"` | M | Frame kind |
| `protocol_major` | integer, `1` | M | Negotiated major, chosen from the client's list |
| `protocol_minor` | integer, `0` for this contract | M | Highest minor the producer implements |
| `incarnation` | 32 lowercase hexadecimal characters | M | 128 unpredictable bits generated once per server start |
| `database_creation` | 64-bit integer | M | The database-creation value recorded identically in every volume header |
| `volumes` | array of volume objects, ascending `volid` | M | The complete set of permanent volumes; temporary volumes are excluded |
| `shared_lru_count` | non-negative integer | M | Number of shared LRU lists for this incarnation |
| `private_lru_count` | non-negative integer | M | Number of private LRU lists for this incarnation |

Volume object: `volid` (integer), `volume_creation` (64-bit integer from the
volume header), `device` (unsigned 64-bit integer) and `inode` (unsigned
64-bit integer) from the server's view of the volume file. Consumers parse
64-bit integers exactly; conversion through floating point is a defect. The
producer never sends a subset of the volume set. If the complete set cannot
fit within the handshake limit, the producer refuses with `identity-oversized`
rather than truncating identity proof.

`error` (server to client, control-frame limit):

| Field | Type | M/O/N | Meaning |
| --- | --- | --- | --- |
| `type` | `"error"` | M | Frame kind |
| `code` | string | M | One of the stable codes below |
| `supported_majors` | array of positive integers | O | Present only with `version-unsupported` |
| `retry_after_ms` | positive integer | O | Present only with `rate-limited` |

Stable codes and connection consequence:

| Code | When | After the frame |
| --- | --- | --- |
| `version-unsupported` | No common major with the client hello | Server closes |
| `busy` | Two clients are already attached | Server closes |
| `incarnation-changed` | `expected_incarnation` present and different | Server closes; client reconnects without expectation and drops retained evidence |
| `identity-oversized` | The complete permanent-volume set cannot fit the handshake limit | Server closes |
| `rate-limited` | A scan request arrives before the producer's 100 ms floor since the last scan start, across all clients | Connection stays open |

`parameter-off` is reserved and never emitted by a v1 producer, because a
disabled producer has no socket. Consumers may accept it defensively. No
`error` frame carries free-text messages, OS error text, paths or process
identities.

`scan_request` (client to server, control-frame limit): `type` only. A
request never carries scope or caps; the bulk-scan caps are the producer's.

`scan_header` (server to client, control-frame limit):

| Field | Type | M/O/N | Meaning |
| --- | --- | --- | --- |
| `type` | `"scan_header"` | M | Frame kind |
| `scan_seq` | positive integer | M | Strictly increasing per incarnation across all clients; not an event sequence |
| `incarnation` | string | M | Must equal the hello's incarnation |
| `started_at_unix_ms` | 64-bit integer | M | Producer wall clock at traversal start; display metadata only, never freshness authority |

`page` (server to client, page-frame limit):

| Field | Type | M/O/N | Meaning |
| --- | --- | --- | --- |
| `type` | `"page"` | M | Frame kind |
| `volid` | integer | M | Volume identifier of the resident page |
| `pageid` | integer | M | Page identifier of the resident page |
| `page_kind` | enum | O | Semantic page kind, see vocabulary |
| `latch_mode` | enum | O | From the single latch-word sample |
| `waiter_present` | boolean | O | From the same latch-word sample |
| `fix_count` | non-negative integer | O | From the same latch-word sample |
| `dirty` | boolean | O | From the single flags sample |
| `flushing` | boolean | O | From the same flags sample |
| `async_flush_requested` | boolean | O | From the same flags sample |
| `to_vacuum` | boolean | O | From the same flags sample |
| `lru_zone` | enum | O | From the same flags sample |
| `lru_list_kind` | enum | O | Decoded from the same flags sample and the hello's list counts |
| `lru_list_index` | non-negative integer | O, N | Kind-local index for `shared` or `private`; `null` for `none` and `invalid` |
| `page_lsa` | LSA object | O, N | Page log position from the buffered page header; `null` when the engine's null-LSA sentinel is sampled |
| `oldest_unflush_lsa` | LSA object | O, N | Oldest unflushed log position; `null` for the null sentinel |

LSA object: `pageid` (64-bit integer) and `offset` (integer). The wire never
carries the packed 64-bit log-position word.

`scan_footer` (server to client, control-frame limit):

| Field | Type | M/O/N | Meaning |
| --- | --- | --- | --- |
| `type` | `"scan_footer"` | M | Frame kind |
| `scan_seq` | positive integer | M | Must equal the header's sequence |
| `page_count` | non-negative integer | M | Exact number of `page` frames emitted, counted before any consumer deduplication |
| `truncated` | boolean | M | True when traversal stopped before visiting every slot |
| `truncation_reason` | enum | O | Present only when truncated: `slot-cap`, `page-cap`, `byte-cap` or `deadline` |
| `ended_at_unix_ms` | 64-bit integer | M | Producer wall clock at footer emission; display metadata only |

### Semantic vocabularies and mapping

15. `page_kind` values are Volmap's existing kebab-case names: `unknown`,
    `file-table`, `heap`, `volume-header`, `volume-bitmap`, `query-result`,
    `extensible-hash`, `overflow`, `oos`, `area`, `catalog`, `btree`, `log`,
    `dropped-files`, `vacuum-data`, plus the reserved `unmapped`. The mapping
    is defined per engine enumerator name, not per ordinal, so one mapping
    serves both branches; the format-aligned branch additionally maps its
    out-of-row enumerator to `oos`, which develop never emits. `unknown` is
    the engine's own initialized or deallocated page type; `unmapped` is any
    enumerator or ordinal without a mapping and must never occur on a
    supported branch.
16. `latch_mode` values are `none`, `read`, `write`, `flush`, `invalid` and
    `unmapped`. The engine's invalidation-in-progress latch state is a real,
    transient state a sampled resident slot can show; it is exposed as
    `invalid`, consistent with the accepted `invalid` LRU zone.
17. `lru_zone` values are `lru1`, `lru2`, `lru3`, `void`, `invalid` and
    `unmapped`. `lru_list_kind` values are `shared`, `private`, `none` and
    `invalid`, decoded exactly per the exact-LRU-membership decision: for an
    LRU-zone slot with packed global index g, `shared` with index g when g is
    below the shared count; `private` with index g minus the shared count when
    g is below the total count; otherwise `invalid` with `null`. A non-LRU
    zone yields `none` with `null`. Indices are incarnation-local.
18. Emission rule: a slot is emitted when its sampled page identity is
    non-null. Whatever the subsequent flags sample shows, including `invalid`
    or `void` zone, the frame is emitted as sampled. Slots with a null
    identity are not resident and are omitted. Pages of temporary volumes are
    resident pages and are emitted; consumers ignore identities outside their
    inspected volumes. Omission means observed non-residency only when the
    footer says the scan was complete.
19. Sampling coherence, stated in the contract: identity is copied first; the
    latch tuple comes from one atomic latch-word load; the flag and LRU tuple
    come from one flags-word load; page kind and page log position come from
    the buffered page header; the oldest unflushed position from the slot.
    Each tuple is coherent within its own load and immediately volatile. The
    frame as a whole and the traversal as a whole are not atomic. VPID
    equality proves neither memory/disk correspondence, commit visibility nor
    durability.
20. Never on the wire: pointers, thread or process identities, raw flag words,
    raw page-type ordinals, packed LRU indices, native list counters, quotas
    or ticks, page contents, hashes of contents, paths or OS error text.

### Truncation and footer reservation

21. The producer reserves capacity for the largest possible canonical footer
    frame before admitting each page frame; the contract publishes that bound
    as a constant. A scan that reaches a cap exactly at complete traversal is
    not truncated. Stopping before visiting every slot is truncated with the
    first reason that applied. The next scan starts after the visited span;
    each slot is visited at most once per scan; no scan merges with another.

### Canonical serializer module

22. Introduce one producer-owned semantic record type made of plain scalars
    and enums, with no engine types, and one canonical encoder that turns a
    hello, error, header, page or footer value into exact frame bytes. The
    encoder enforces frame limits and reports a would-exceed condition instead
    of emitting. It compiles in every build mode without server-only
    dependencies so a unit test can build it directly, following the existing
    pattern of testing a dependency-free source alongside its Catch2 case.
    Direct formatting is sufficient for flat frames; using the already-bundled
    JSON library is permitted; adding a dependency is not.
23. The module owns the page-kind name mapping by enumerator, the latch tuple
    mapping from decoded latch values, and the LRU tuple arithmetic from a
    zone, a packed global index and the two list counts. Extracting those
    inputs from engine words remains engine-side stage-two work; the module
    never includes engine internals.
24. The same encoder generates every canonical corpus stream from a semantic
    input file. Malformed cases are derived by documented mutation of a
    canonical stream and stored as bytes. Hand-authored canonical bytes are
    not permitted; if a stream cannot be generated by the encoder it is not
    canonical.

### Corpus format, manifest and vendoring

25. The corpus is a directory of cases, one subdirectory each, named by a
    stable case identifier. Every case holds the exact server-to-client bytes
    the consumer must decode and a machine-readable expected outcome. Cases
    generated by the encoder also hold their semantic input. Cases that
    include a client turn also hold the canonical client frames that elicited
    the stream, so the consumer's own encoder can be checked.
26. The expected outcome names the case, its category, one outcome from
    `accepted-complete`, `accepted-partial`, `discarded`, `refused` or
    `connection-closed`, a discard reason from a fixed list published in the
    contract when discarded, the refusal code when refused, the decoded scan
    summary (sequence, page count, truncated, reason), the normalized decoded
    pages, the list of ambiguous duplicate identities, and the count of
    ignored unknown fields and frames. Outcome vocabulary belongs to the
    corpus format, not the wire.
27. Required coverage, each as at least one case: complete empty scan;
    complete scan exercising every enum value including `oos`, `invalid` and
    `void` zones, `invalid` latch mode, `flush` latch mode, waiter present,
    private and shared list indices, null and non-null log positions; one
    truncated scan per reason; additive unknown fields in hello, header, page
    and footer; an unknown frame kind inside a scan; duplicate identities;
    two consecutive scans on one connection with increasing sequence;
    `rate-limited` followed by a later valid scan; every refusal code as a
    hello or request reply; a hello with additive fields; a hello missing a
    mandatory field; a hello exceeding the handshake limit; missing footer at
    end of stream; page count over and under; footer sequence mismatch;
    header incarnation mismatch; non-increasing sequence across scans;
    invalid JSON; invalid UTF-8; a page frame of exactly 4,096 bytes accepted
    and 4,097 rejected; depth 16 accepted and 17 rejected; a page frame
    missing `volid` or `pageid`; a frame after the footer.
28. Large boundary cases — 65,536 pages accepted, 65,537 rejected, and a scan
    exceeding 64 MiB — are described by a deterministic generation recipe in
    the contract plus the SHA-256 of the generated stream and its expected
    outcome, instead of stored bytes. Both repositories generate and hash;
    the corpus stays small enough to vendor.
29. Integrity manifest: a checksum file in the standard `sha256sum` format
    listing every corpus file except itself, sorted by path, so any Unix host
    verifies integrity with standard tooling. The aggregate corpus hash is the
    SHA-256 of that checksum file's bytes. A small JSON manifest records the
    contract version, an integer corpus revision and the aggregate hash. The
    manifest carries no commit identifier; commits are recorded by the
    vendoring side and by the delivery evidence manifest.
30. Vendoring: Volmap copies the corpus directory verbatim, records the
    aggregate hash and the producer commit that carried it, and its offline
    test recomputes the checksum file and compares the aggregate. The
    producer's own offline test does the same in the CUBRID repository. The
    aggregate hash is the authority for content; the commit is provenance.
    When the develop port lands, its commit is added as a second provenance
    entry under the same hash.
31. Change control: any byte change to the corpus increments the corpus
    revision and produces a new aggregate hash, and requires matching offline
    evidence in both repositories before delivery. Additive schema changes
    increment the protocol minor; incompatible changes start a new major in a
    new contract directory. A semantic disagreement discovered during
    implementation is resolved by amending the relevant decision ticket, never
    by adjusting syntax or adding consumer exceptions.

## Testing Decisions

1. A good test here observes external behavior at two seams and nothing
   else: bytes out of the canonical encoder for a semantic value in, and the
   corpus's expected outcomes for its stored bytes. Tests do not assert on
   private helper names, buffer strategies or call order.
2. The cross-repository seam is the corpus itself. Volmap's production decoder
   consumes it in ticket 02; the CUBRID encoder reproduces its canonical
   streams here. Both sides verify the same aggregate hash offline. No test
   requires a network, a running server or the other repository.
3. The producer-internal seam is the canonical serializer's interface:
   semantic record in, frame bytes or would-exceed out. The later BCB scan is
   tested against this same interface, so stage-two work adds no new
   serializer tests. Prefer this existing seam over any scan-level fixture.
4. CUBRID unit tests, in a new Catch2 module registered like the existing
   modules with its own option flag, cover: byte-exact reproduction of every
   canonical corpus stream from its semantic input; canonical form rules
   (key order, no whitespace, integer and literal forms); frame-limit
   enforcement at 4,095/4,096/4,097 and 65,535/65,536/65,537 bytes; footer
   reservation bound versus the largest constructible footer; page-kind
   mapping for every enumerator on the branch under test, asserting `oos`
   only where the enumerator exists; latch and LRU tuple mapping including
   the shared/private/invalid bounds; null versus non-null log positions;
   `unmapped` for out-of-mapping inputs; and corpus integrity, recomputing
   the checksum file and comparing the aggregate hash.
5. Prior art: the JSON-tree unit test builds its dependency-free source
   directly into the test executable without linking an engine library; the
   string-buffer and packing tests show module registration. Volmap's pinned
   page corpus pins fixture bytes by SHA-256 and documents reproduction in a
   manifest and README; the conformance corpus follows that shape.
6. Proof of execution: record the test executable's own case listing and its
   summary line with case and assertion counts, for the format-aligned branch
   and again for the develop port. A successful configure, build or an empty
   test-runner invocation is not evidence. Registration with the test driver
   is desirable but does not replace running the binary and capturing counts.
7. Public verification instructions use standard project concepts: the
   CMake unit-test option, building the test target and running the
   executable. Personal task-runner recipes and workspace paths do not
   appear in the contract, corpus README, commit messages or PR text.
8. Consumer-side decoder, chunking, stall, admission, cache, HTTP and browser
   tests belong to Volmap ticket 02 and are not repeated here; the corpus is
   designed so those tests can use its streams verbatim.

## Out of Scope

- The bounded engine producer: startup-only parameter, private daemon, socket
  directory and permissions, stale-socket handling, peer credentials, engine
  identity collection, incarnation generation, lock-free BCB traversal,
  rotation, rate limiting and disconnect-on-stall. These are the next
  delivery-plan stage and reuse the serializer delivered here.
- The external shell testcase, real-socket debug/release checks, controlled
  known-VPID fixtures, the develop producer port's runtime evidence, and every
  performance, CPU, memory, completeness or interference measurement.
- Any Volmap implementation: decoder, broker, HTTP, browser, vendoring commit.
  This specification only defines what Volmap vendors.
- Point inspection, page-image capture, digests, DWB or TDE comparison,
  consistency classification, AOUT history, transition or flush events, TUI
  parity, SHOW-statement fallback, shared memory or remote transport.
- JIRA description changes, commits, pushes, PR creation or any other external
  publication. Those are separate authorized workflows.
- Reopening accepted semantics. Concrete-syntax clarifications are listed
  below for confirmation; none changes an accepted decision.

## Further Notes

### Concrete-syntax clarifications (accepted 2026-09-08)

These implement accepted semantics where the decisions left the syntax open.
Each is additive and consumer-safe under the unknown-value rules above. The
user accepted all ten on 2026-09-08; ticket 07's final contract review marks
each confirmed and treats any later deviation as a decision-ticket amendment.

1. `invalid` is a `latch_mode` value. The engine has a real invalidation
   latch state that a sampled resident slot can show; hiding it would require
   either dropping the frame or lying. This parallels the accepted `invalid`
   LRU zone.
2. `unmapped` is a reserved value in `page_kind`, `latch_mode` and
   `lru_zone` for internal values with no mapping, so that "engine initialized
   page" (`unknown`) and "producer has no name for this" stay distinct and no
   ordinal ever leaks.
3. `expected_incarnation` in the client hello is the trigger for the accepted
   `incarnation-changed` code, which otherwise has no producible condition.
4. `identity-oversized` is an additive refusal code for the accepted rule that
   an identity set exceeding the handshake limit refuses attachment rather than
   truncating proof. The decisions specified the outcome but no code.
5. `truncation_reason` and `retry_after_ms` are optional additive fields that
   make accepted behaviors (honest partial coverage, the 100 ms floor)
   observable without new semantics.
6. Page-kind names adopt Volmap's existing kebab-case vocabulary rather than
   engine enumerator spellings, so the overlay and disk facts share terms.
7. Log positions are objects of `pageid` and `offset`; the null-LSA sentinel is
   JSON `null`. The packed word is never sent.
8. Resident pages of temporary volumes are emitted. Filtering them would be a
   semantic change to "resident set"; consumers already ignore identities
   outside their inspected volumes.
9. Unknown frame kinds inside a scan are ignored, not fatal, to allow additive
   frame kinds within a major version. Unknown kinds outside a scan remain
   protocol violations.
10. The canonical form binds only the producer. Consumers must accept any
    valid JSON form within limits; byte-strictness on the consumer side would
    couple Volmap to formatting rather than semantics.

### Seams

The shared seam is the corpus: exact bytes plus expected outcomes plus an
aggregate hash. The producer-internal seam is the canonical serializer's
semantic-record interface, chosen so that the later BCB scan, daemon and
socket work add engine glue but no new serialization or test surface. No
other seam is introduced. The contract document, corpus README and vendoring
notes are the only documentation deliverables.

### Tracker placement

The originating handoff prohibits JIRA changes, and the CUBRID worktree has no
configured local tracker. This specification therefore lives in Volmap's
configured local tracker beside the other artifacts of this effort, under its
own feature slug, while describing work performed in the CUBRID repository.
If the user prefers a tracker inside the CUBRID worktree, run the tracker
setup skill there and move this file; nothing else depends on its location.

### References

- [Producer handoff entry](../pgbuf-overlay/handoff/cubrid-entry.md),
  [handoff index](../pgbuf-overlay/handoff/README.md) and
  [delivery plan](../pgbuf-overlay/handoff/delivery-plan.md).
- Decision authorities: [transport](../pgbuf-overlay/issues/03-choose-transport-channel.md),
  [gating](../pgbuf-overlay/issues/04-define-gating-matrix.md),
  [wire v1](../pgbuf-overlay/issues/05-define-wire-contract-v1.md),
  [security](../pgbuf-overlay/issues/08-set-security-posture.md),
  [verification](../pgbuf-overlay/issues/11-define-verification-strategy.md),
  [exact LRU membership](../pgbuf-overlay/issues/14-expose-exact-lru-list-membership.md),
  [budgets](../pgbuf-overlay/issues/15-set-overlay-resource-budgets.md),
  [branch alignment](../pgbuf-overlay/issues/02-choose-target-branch.md).
- Consumer authority: [Volmap implementation specification](../pgbuf-overlay-implementation/spec.md)
  and [ticket 02](../pgbuf-overlay-implementation/issues/02-observe-selected-page-securely.md).
- Domain vocabulary: [CONTEXT.md](../../CONTEXT.md) and
  [ADR-0006](../../docs/adr/0006-runtime-observations-are-loopback-web-capabilities.md).
- Source facts checked on the analysis worktree at
  `cd593bcf2d8643b4698f1cb311c4c23af23a9d57`: the volume header carries both a
  database-creation and a per-volume creation value; the latch enum includes
  an invalidation state; the existing lock-free status scan samples identity
  and flags without slot locks; the JSON library is already bundled; the
  unit-test tree builds dependency-free sources directly into Catch2 targets.
- CUBRID issue draft for the later JIRA workflow:
  `CBRD-27398-pgbuf-overlay_cd593bc_codex.md` in the configured issue-draft
  repository; unchanged by this specification.

Implementation tickets 01 through 07 under `issues/` were created from this
specification on 2026-09-08 with the user's approval of granularity and
blocking edges: 01 → 02 → 03, then 04 and 05 in parallel off 03, 06 after 04,
and 07 after everything. Work the frontier with `/implement` and `/tdd` at the
two seams above, in a fresh CUBRID implementation worktree, registering the
work as tracked before code changes begin. Return the corpus aggregate hash,
both producer commits and the captured test execution evidence to Volmap
before ticket 02 starts.
