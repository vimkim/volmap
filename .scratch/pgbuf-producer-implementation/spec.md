# Bounded CUBRID page-buffer observation producer

Status: ready-for-agent

Scope: CUBRID producer and producer-owned conformance corpus for CBRD-27398.
Published in the configured cross-repository local Markdown tracker on
2026-09-09. Companion Volmap implementation has its own specification and
tickets. This specification records requirements, not implementation or
verification results.

## Problem Statement

An operator can inspect CUBRID volume files and aggregate buffer statistics,
but cannot obtain bounded, page-addressable evidence of buffer residency,
latch/fix state, dirty/flushing state and exact replacement-list membership
for a cooperating Volmap viewer. Raw engine layouts would couple the viewer
to a particular build and disclose inappropriate data. An unbounded or invasive
inspector could also interfere with the database it is supposed to observe.

The operator needs optional page-buffer observation associated with the correct
database and server incarnation, with explicit scan coverage and limits.
Observed disk state and runtime page observations have independent timing;
matching VPID, recency or a state bit cannot establish page image correspondence,
transaction visibility or durability.

## Solution

Provide an explicitly enabled, default-off local producer in cub_server. A
permitted same-account client connects through a private Unix stream socket,
negotiates a versioned semantic protocol, verifies persistent-volume identity,
and requests a bounded resident-set scan. The producer returns scalar
observations bracketed by a scan header and a valid completion footer.

Ordinary database operation continues when observation is disabled or socket
activation fails. Slow clients cannot retain page protection or block database
workers. Complete, partial and broken scans have distinct meanings. The
producer supplies one stable contract for the format-aligned integration
branch and the eventual develop implementation.

## User Stories

1. As a database operator, I want observation disabled by default, so that ordinary startup creates no inspector endpoint or scanning work.
2. As a database operator, I want explicit startup configuration, so that enabling observation is deliberate and reproducible.
3. As a database operator, I want the same state-only capability in supported release and debug server builds, so that diagnosis does not require a special instrumented engine.
4. As a database operator, I want socket activation failure to leave the database running, so that optional diagnostics cannot prevent service startup.
5. As a database operator, I want the socket location reported locally, so that I can explicitly configure an authorized client.
6. As a database operator, I want owner-only directory and socket permissions, so that unrelated local accounts cannot access observation data.
7. As a database operator, I want exact effective-UID peer verification without root or group exceptions, so that the authorization principal is unambiguous.
8. As a database operator, I want unsafe or active socket entries preserved, so that startup cannot destroy another resource.
9. As a database operator, I want shutdown to remove only the socket this incarnation created, so that pathname replacement cannot cause unrelated deletion.
10. As a client implementer, I want version negotiation and stable refusals, so that incompatible or overloaded attachment fails predictably.
11. As a client implementer, I want complete persistent-volume identity evidence, so that similarly named or copied databases cannot be confused.
12. As a client implementer, I want an unpredictable server-incarnation identifier, so that restart invalidates all older observations.
13. As an operator, I want resident observations keyed by VPID, so that I can relate them to a selected physical page.
14. As an operator, I want latch mode, waiter presence and fix count sampled coherently, so that those three values describe one latch-word reading.
15. As an operator, I want dirty, flushing, asynchronous-flush-requested and to-vacuum state, so that I can inspect sampled engine conditions.
16. As an operator, I want semantic page kind and log positions, so that physical type ordinals and private layouts do not become client dependencies.
17. As an operator, I want exact shared/private LRU membership and zone from one flags sample, so that list detail is internally coherent within that sample.
18. As a client implementer, I want incarnation-scoped LRU topology counts, so that kind-local indices can be interpreted and validated.
19. As an operator, I want an explicit scan interval, so that I understand that traversal is not an atomic pool snapshot.
20. As a client implementer, I want sequence, count and truncation framing, so that I can validate a capture before publishing it.
21. As an operator, I want complete-scan omission distinguished from partial unknown, so that resource limits cannot fabricate nonresidency.
22. As a client implementer, I want repeated VPIDs treated as ambiguous evidence, so that concurrent replacement cannot silently become last-write-wins.
23. As an operator, I want successive truncated scans to rotate through the pool, so that the same slots are not permanently excluded.
24. As an operator, I want observations to leave residency, replacement and accounting unchanged, so that inspection does not create the condition it reports.
25. As a database operator, I want bounded client admission, scan frequency, output memory and elapsed work, so that observation cost remains controlled.
26. As a database operator, I want stalled or disconnected clients cancelled promptly, so that they cannot obstruct page-buffer work or shutdown.
27. As a client implementer, I want additive optional fields supported within fixed framing limits, so that compatible evolution does not require synchronized upgrades.
28. As a maintainer, I want a producer-owned conformance corpus tested independently by both repositories, so that protocol drift is caught offline.
29. As a maintainer, I want deterministic decoding and scan-limit tests, so that subtle boundary failures have reproducible oracles.
30. As a release reviewer, I want controlled real-server residency, dirty and eviction checks, so that serializer-only tests cannot stand in for engine correctness.
31. As a release reviewer, I want debug and release evidence on both delivery targets, so that semantic compatibility does not hide build or lifecycle differences.
32. As a release reviewer, I want exact-commit resource and performance evidence, so that the cost of active observation is demonstrated rather than assumed.
33. As an operator, I want only sanitized structural observations, so that application bytes, process identities and private memory are not disclosed.
34. As a maintainer, I want missing or inconclusive verification to remain visible, so that delivery cannot be approved from incomplete evidence.

## Implementation Decisions

### Ownership and engine integration

1. CUBRID owns the semantic producer, protocol schema/corpus, parameter,
   socket lifecycle and producer tests. Volmap owns its adapter, broker,
   HTTP projection and browser. Keep the producer behind a small lifecycle
   interface and a private bounded sampling boundary; callers do not learn
   BCB layouts or protocol serialization internals.
2. Reuse existing page-buffer scalar-scan and server daemon conventions.
   Integrate initialization after required buffer, identity and thread
   services are available; cancel and finish producer work before those
   services are destroyed. Socket I/O belongs to the dedicated inspector
   daemon. Serialization and writes use producer-owned scalar samples and
   never retain a BCB mutex or page latch.
3. Observation performs no page fix, missing-page load, page-content copy,
   replacement-list traversal or mutation, accounting update, disk-content
   read, DWB probe or TDE operation. Identity acquisition uses the existing
   database/volume metadata and OS file identity, separately from scanning.
   Revalidate scalar access/lifetime assumptions on each implementation base;
   the existing scan is precedent, not proof that every added field is safe.
4. Add startup-only Boolean enable_pgbuf_inspector, default false, with
   PRM_FOR_SERVER and PRM_HIDDEN only. Read it once during initialization.
   Add no user-change, reload, client or forced-server synchronization flags.
   Disabled means neither daemon nor socket and no hot-path parameter reads.
5. Add no v1-specific CMake option or hand-defined feature macro. The
   state-only producer is compiled into supported Unix SERVER_MODE builds,
   including Release, RelWithDebInfo, Debug and OptDebug. Windows and
   non-server binaries expose no endpoint. Linux SO_PEERCRED is the accepted
   authentication mechanism; platform support cannot silently substitute a
   weaker peer policy. Future protected capture capabilities remain separate.
6. Preserve CUBRID error handling, source formatting and include conventions;
   reuse available JSON facilities without adding a runtime dependency.

### Private attachment and lifecycle

7. Use AF_UNIX SOCK_STREAM with newline-delimited JSON. Use the existing
   CUBRID Unix-socket root, a dedicated owner-only pgbuf-inspector directory
   at mode 0700, and a socket at mode 0600. Derive a bounded opaque database
   key from canonical database path plus creation identity. Report the
   resolved socket location locally; client attachment remains explicit.
8. Authenticate the connected client with SO_PEERCRED before protocol exchange:
   its effective UID must exactly equal the server's. Unauthorized connections
   close without observation data. The consumer also checks server credentials
   and directory/socket ownership; neither side grants root or group exceptions.
9. Preserve symlinks, non-sockets, wrong-owner entries and any socket accepting
   a connection. Reclaim only a same-owner socket positively confirmed stale
   by a failed connection attempt; an indeterminate failure is not permission
   to unlink. Track the created socket's identity and remove it at shutdown
   only if the pathname still identifies that exact socket.
10. Path-length, ownership, permissions, reclamation, bind or listen failure
    leaves the inspector unavailable for the entire incarnation. Emit a
    prominent stable local startup error and continue database startup.
    Do not retry binding in the background. Shutdown cancels accepted clients
    and releases producer resources before page-buffer teardown.
11. Handshake identity covers database creation and the complete persistent
    volume set: each volume's volid, volume creation, device and inode.
    Producer-only temporary volumes do not participate in identity proof.
    Names or paths alone cannot prove attachment. Refuse oversized identity
    sets rather than truncate the proof. Supply an unpredictable incarnation
    identifier distinct from persistent identity. Identity mismatch is refused
    with stable consumer reason identity-mismatch.
12. Paths, UID/GID/PID and raw OS errors are not observation payloads or
    downstream user-facing disclosure. The consumer may display a safe
    identity fingerprint, abbreviated incarnation, protocol, verification
    state, times and stable reasons. Socket absence alone cannot distinguish
    a disabled server from failed/unavailable activation.

### Semantic scan and protocol contract

13. Support bulk resident-set scans only. VPID is the record identity; do not
    expose slot identity as persistent page identity. A scan is a best-effort
    pool traversal, not an atomic record, pool snapshot or event stream.
14. Supply volid, pageid, latch_mode (none/read/write/flush), waiter_present,
    fix_count, dirty, flushing, async_flush_requested, to_vacuum, page_lsa,
    oldest_unflush_lsa and semantic page_kind. Load the atomic latch word once
    and decode its three fields together; separate getters that reload the
    word do not satisfy this requirement. Never serialize raw structures,
    pointers, packed flags, application bytes or PAGE_TYPE ordinals.
15. Supply lru_zone (lru1/lru2/lru3/void/invalid), lru_list_kind
    (shared/private/none/invalid), and nullable lru_list_index. Decode the
    entire LRU tuple from one flags sample. For an LRU zone, a packed index
    below shared_lru_count maps to shared; the next private_lru_count indices
    map to private after subtracting shared_lru_count. Out-of-range membership
    maps to invalid/null. Non-LRU zones map to none/null. Handshake topology
    counts and indices are incarnation-scoped. Native list counters, quotas,
    thresholds, ticks and pointers are excluded.
16. Map page kinds through a wire-owned vocabulary on each branch, including
    the reserved oos kind that develop does not emit. Contract consolidation
    must define safe unknown/invalid values, LSA representation and exact
    numeric encodings without leaking internal ordinals or losing precision.
17. A header carries monotonic scan_seq per incarnation and scan start time;
    records follow; a footer carries end time, exact emitted-record count and
    explicit truncation. Bind framing to the negotiated incarnation and
    sequence. Count records before consumer deduplication. No per-record
    timestamp is invented. Source wall-clock times describe the interval;
    client freshness uses its independent conservative monotonic age rules.
18. A complete valid scan supports observed nonresidency by omission for an
    evaluated requested VPID. A partial gap or unevaluated request is unknown.
    Repeated VPIDs are ambiguous/unknown, never last-write-wins. A missing
    footer, malformed capture or mismatch in count/sequence/incarnation cannot
    publish a new capture. Valid truncated captures remain explicitly partial.
    Never merge scans to establish completeness or absence.
19. Use snake_case fields, lowercase semantic enum strings and additive-only
    evolution within a major version. Unknown optional fields are ignored
    within resource limits. Optional page-state evolution does not make
    required identity, framing or count fields optional.
20. Stable protocol refusal codes are version-unsupported, busy, rate-limited
    and incarnation-changed. The disabled producer does not emit parameter-off
    because it has no listener; consumers may recognize it defensively.
21. The first implementation deliverable is the exact versioned schema and
    conformance corpus: handshake/request/header/record/footer/error shapes,
    mandatory fields, numeric and null representations, enum fallbacks and
    expected validation outcomes. This is concrete encoding work under the
    accepted semantics, not an invitation to change the design. Pin the
    consumer's vendored corpus by revision and content hash; test both sides
    independently offline before cross-repository delivery.

### Bounded execution

All byte limits are binary units and include framing. Enforce limits before
unbounded allocation; the whole-scan byte allowance is not a buffering budget.

| Resource | Accepted bound |
| --- | --- |
| Concurrent clients | 2 maximum |
| Server-side scan interval | 100 ms minimum |
| Visited slots and emitted records | 65,536 each per scan |
| Total scan bytes | 64 MiB, including reserved footer capacity |
| Record/control frame | 4 KiB including newline |
| Handshake | 64 KiB; complete identity proof required |
| JSON nesting | 16 maximum |
| Traversal and serialization | 100 ms elapsed, checked between slots |
| Output buffering | 64 KiB |
| Write stall | Disconnect after 250 ms without progress |
| Connect plus handshake | 500 ms end-to-end attachment deadline |
| Whole scan exchange | 2 s, including draining and footer |

22. Traverse each slot at most once per scan. Reserve footer bytes before
    admitting another record. Stop truthfully at any cap; reaching a limit
    exactly after complete traversal need not imply truncation. Advance the
    next start beyond the visited span after truncation and prove progress
    on stable pools. Larger pools do not automatically expand budgets.
23. Traversal elapsed time includes backpressure from traversal start and is
    checked between slots. This is not a hard real-time guarantee. Drain/footer
    transmission stays within the whole-exchange deadline. Deadlines never
    authorize blocking a database worker or retaining page protection.
24. Use bounded streaming and cancellation rather than assembling a 64 MiB
    capture in memory. On disconnect, stall, shutdown or failed exchange,
    stop unnecessary work and release resources. An incomplete stream cannot
    be turned into a successful partial capture by its consumer.

### Delivery boundaries

25. Revalidate source and worktree state before implementation. The accepted
    sequence iterates from the Volmap format pin e1e651d, performs format-aligned
    integration, then ports the producer to develop. Use isolated implementation
    branches and preserve unrelated work. This analysis worktree's HEAD is
    evidence, not an implicit branch selection. Semantic protocol compatibility
    does not establish Volmap support for develop's disk format.
26. Deliver the producer-owned corpus, engine unit tests and companion external
    shell testcase with the engine change. Record the actual external testcase
    repository revision. Cross-repository integration and performance require
    the separately implemented consumer; deterministic producer work can
    proceed first. Source/JIRA/PR publication is a later workflow.

## Testing Decisions

1. Use the public Unix-socket exchange as the principal behavioral boundary.
   Focused scan/serializer tests cover deterministic semantic and budget
   boundaries underneath it. These layers preserve the already accepted
   verification decision; a seam check was also presented during synthesis.
   Assert observable records, coverage, refusals, timing bounds and resource
   release rather than helper names, object layouts or incidental call order.
2. Existing aggregate BCB scanning is sampling prior art; existing Catch2
   2.11.3 packing and JSON-builder executables are unit-test prior art. The
   producer and its test target are new work. Register or directly invoke
   executable tests and record named cases with nonzero execution counts;
   build success or an empty ctest run does not establish coverage.
3. Deterministic cases cover both branch page-kind mappings; valid and invalid
   latch/LRU tuples; topology bounds; empty, complete and truncated scans;
   count/framing; footer reservation; every cap below/at/above its boundary;
   rotation progress; deadlines; cancellation and stalled writes. Inject time
   and controlled scalar samples privately where needed. Keep real transport
   timeout tests in addition to simulated time.
4. The shared corpus covers valid, partial, malformed, additive-field,
   missing-mandatory-field, invalid JSON/depth, oversize, count/sequence/
   incarnation mismatch and duplicate-VPID scenarios. Assert expected
   semantics independently through the actual producer encoder and consumer
   decoder. Chunked/coalesced stream reads and missing-footer EOF belong in
   real-parser socket tests. Corpus changes require updated cross-repo evidence.
5. Real-socket engine cases cover default-off absence, enabled attachment,
   exact-UID authorization, directory/socket permissions, copied-database
   identity mismatch, safe stale recovery, active/unsafe path preservation,
   activation failure with successful database startup, two-client admission,
   rate limiting, partial framing, disconnect, shutdown and restart. Prove
   socket replacement cannot cause unrelated cleanup. Credential tests need
   an appropriate isolated environment; its absence is missing evidence.
6. Controlled test-only engine fixtures establish known VPIDs and hold
   residency/dirty conditions across observation with bounded synchronization.
   Eviction cases prove removal and prevent refixing through a complete scan.
   Do not use sleeps as oracles or let the observer fix/mutate a page. Failed
   preconditions are inconclusive. Add no shipped control endpoint; test
   machinery cannot replace unmodified release attachment/lifecycle checks.
7. Require debug and release producer checks on develop and debug and release
   full integration on the agreed format-aligned branch. Deliver the companion
   external shell case and actual invocation evidence. Missing private-suite
   access is not passing, and hosted automation is not assumed to exist.
8. On a recorded dedicated host, measure release builds at 512 MiB, 1 GiB and
   4 GiB pools with 16 KiB pages under idle, read-heavy, write/flush-heavy and
   churn workloads. The shared integration matrix includes 1/8/32 browser tabs,
   stalls, viewport rotation, pause/hidden, clock changes and restart. Consumer
   and browser gates remain owned by their companion specification.
9. Use ten paired runs per performance case, 60 s warmup and at least 5 min
   measurement, extended to 100,000 transactions where applicable. Matched
   pairs preserve workload, database state, concurrency and build except
   inspector activity. The 95% confidence upper bound must meet at most 2%
   throughput loss and 5% p99 transaction-latency increase. Inconclusive is
   not passing; do not average away a failing case.
10. At default consumer cadence, producer CPU is at most 20% of one logical
    core and incremental peak RSS at most 16 MiB. For the idle/read-heavy
    1 GiB reference require at least 99% complete scans and end-to-end p95
    refresh at most 250 ms. Larger pools require truthful partial coverage.
    Measure completeness separately from latency. Record CPU-time/wall-time,
    allocation peaks and RSS separately. Measure unmodified versus parameter-off
    builds and enabled/no-demand behavior separately from active overhead.
11. The full delivery gate also retains companion thresholds: broker CPU 50%
    of one core, incremental RSS 192 MiB, browser 32 MiB per tab, broker
    accounted allocation 128 MiB, cached HTTP p95 25 ms, concurrent disk
    inspection p95 regression 5%, and actual 10,000-cell browser p95 interaction
    100 ms with semantic/manual accessibility checks. Producer unit success
    does not discharge these shared delivery obligations.
12. Maintain a gate manifest mapping every invariant and budget to its owner,
    test, exact producer/consumer commits, corpus revision/hash, build mode,
    external testcase revision, environment, preconditions, commands, executed
    counts, result and raw artifacts. Performance evidence includes hardware,
    dataset, concurrency, seeds, baselines, samples and confidence calculation.
    Missing/skipped/failing/inconclusive required evidence keeps delivery open.
    Changes invalidate affected evidence. Tuning is allowed within accepted
    limits; weakening a limit requires an explicit design revision.

## Out of Scope

- Volmap adapter, broker, HTTP, browser or kernel-cache implementation; these
  have their own tickets and remain cross-repository integration dependencies.
- Point InspectPage, protected resident-page inspection, raw page capture,
  memory/main-volume/DWB digests, TDE normalization and consistency classification.
- AOUT collection/history/re-enablement, flush-transition hooks, event buffers,
  changefeeds, replay, causal timelines and per-fix instrumentation. Sampled
  flushing fields remain; differences between scans prove no event timing,
  count, cause or durability.
- Native LRU telemetry, thread-holder identities, pointers, stacks and
  application values; stop-the-world or transactional snapshots.
- SHOW fallback, shared-memory export, TCP/remote inspector endpoints,
  automatic socket discovery, HTTP authentication or TLS.
- Runtime TUI parity, inspection-graph facts/revisions, exports and any claim
  of page-image correspondence or transaction visibility from state alone.
- Engine changes, test execution, commits, pushes, JIRA upload or PR creation
  during this specification synthesis.

## Further Notes

This producer specification complements the existing Volmap implementation
specification. It consolidates the resolved map for implementation-ticket
creation without reopening its decisions. Ready-for-agent describes readiness
to create and implement tickets under declared dependencies; it does not mean
the protocol corpus, producer, shell tests, upstream socket approval or release
evidence exists.

The source was checked at CUBRID cd593bcf2d8643b4698f1cb311c4c23af23a9d57.
The scan precedent reads scalar BCB state without a BCB mutex, and latch getters
individually load the atomic word: the producer must use one sample for the
whole tuple. The existing standalone JSON-builder test target does not itself
register a ctest test. These observations guide implementation and do not
prove a new collector safe or tested.

Exact JSON syntax, private helper names and lifecycle wiring are implementation
deliverables constrained above. No byte-for-byte schema or benchmark result
is claimed by this synthesis. The earlier loose handoff's open gating/security
status is superseded by the resolved decisions linked below.

Authoritative references:

- [Resolved decision map](../pgbuf-overlay/map.md) and [producer handoff](../pgbuf-overlay/handoff/cubrid-entry.md).
- [Gating](../pgbuf-overlay/issues/04-define-gating-matrix.md), [wire semantics](../pgbuf-overlay/issues/05-define-wire-contract-v1.md), and [LRU membership](../pgbuf-overlay/issues/14-expose-exact-lru-list-membership.md).
- [Security and identity](../pgbuf-overlay/issues/08-set-security-posture.md), [budgets](../pgbuf-overlay/issues/15-set-overlay-resource-budgets.md), and [verification](../pgbuf-overlay/issues/11-define-verification-strategy.md).
- [Branch alignment](../pgbuf-overlay/issues/02-choose-target-branch.md) and [delivery ownership](../pgbuf-overlay/handoff/delivery-plan.md).
- [Companion Volmap specification](../pgbuf-overlay-implementation/spec.md).
- [Domain glossary](../../CONTEXT.md) and [runtime capability ADR](../../docs/adr/0006-runtime-observations-are-loopback-web-capabilities.md).
- [Local producer issue draft](/home/vimkim/gh/my-cubrid-jira/issues/CBRD-27398-pgbuf-overlay_cd593bc_codex.md).
- [Local observability prior art](/home/vimkim/gh/my-cubrid-docs/pgbuf-analysis/e6ed61e_claude/06-misc-observability.md).

Next: `/to-tickets` creates a separate producer implementation graph, beginning
with contract/corpus consolidation and preserving integration/port/release
dependencies. Resolved Wayfinder decision tickets remain historical records.
