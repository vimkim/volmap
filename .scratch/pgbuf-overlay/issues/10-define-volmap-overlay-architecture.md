Type: grilling
Status: resolved
Blocked by: 05, 06, 08

# Define the volmap overlay architecture

## Question

Given wire contract v1, the domain terms, and the security posture, fix the volmap-side architecture (consult codebase-design for the seam):

1. The overlay store — a sibling holder beside `Arc<LiveSource>` in `WebState` with its own `tokio::sync::watch` channel and poll task; strictly out of the inspection graph; never advancing snapshot generations or touching cursors.
2. Client — UDS connection lifecycle, reconnect/backoff, incarnation-change handling (server restart discards the overlay, never mixes observations across incarnations).
3. Publication — a separate `/api/v1` overlay resource vs an extra field on `WatchProjection`'s long-poll (SSE stays rejected); polling cadence; admission (its own semaphore like watchers).
4. Skew honesty — every overlay response carries its own observation time and capture token; the UI never implies the overlay and the disk facts are one instant.
5. Degradation — parameter off, socket absent, handshake refused, or version mismatch all degrade to the overlay being absent with a diagnostic-style explanation, not an error page.
6. Module seam — where the inspector client lives (a new `src/` module mirroring `follow.rs`'s role) and what its testable interface is (the in-memory adapter from the design reference).

## Comments

- 2026-09-08 evidence: the live-web implementation specification already
  defines a runtime broker and separate no-store runtime resources. Its
  selected/visible-page request model must be reconciled with the finalized
  bulk-only producer wire, rather than copied into producer point requests.
- Current code has no engine-runtime broker: `src/web.rs:67` holds the disk
  source and inspection/watch admission; `src/web.rs:558` watches disk
  generations; `src/follow.rs:120` owns disk-generation notifications.
  Runtime HTTP responses need their own semantic envelope instead of the
  inspection outcome/diagnostics envelope at `src/web.rs:423`.
- Initial user decision frontier: shared broker and coalesced scan cache;
  treatment of valid truncated scans versus broken streams; scan-interval
  timing instead of invented per-page timestamps. Scheduling, lifecycle, and
  numerical budgets follow after those choices.

## Answer

### Accepted architecture — 2026-09-08

The user accepted the initial recommendations:

- One shared runtime broker per attached serve session owns one producer
  connection and a bounded latest-scan cache. It coalesces browser demand and
  serves selected/visible-page subsets through the existing separate runtime
  HTTP resources. Disk watch remains independent.
- A valid truncated scan publishes its observed records as partial evidence;
  omitted pages are unknown. Do not fill gaps from older scans as if their
  records belonged to the new scan. A broken stream without a valid footer is
  discarded; the previous usable scan retains its original age.
- Expose scan start/end and an incarnation-bound capture identity. Individual
  page capture time is unavailable in wire v1; do not synthesize it from HTTP
  response time. The glossary now accommodates source-supplied capture
  intervals without requiring per-page timestamps.

The user also accepted the scheduling, lifecycle, coverage, and budget-ticket
recommendations below. This resolves the architecture; numerical resource
limits remain a separate blocking decision before handoff.

### Module and publication

Keep the broker as a private runtime module alongside the disk-follow module,
held separately in `WebState`. Its interface provides capability metadata and
bounded observations for a validated scope. It hides producer framing,
identity verification, connection lifecycle, scan assembly, cache replacement,
coalescing, and limits from HTTP handlers and browser code. A private source
adapter seam accepts the UDS producer or a deterministic simulated source;
inject clock/scheduling dependencies so both exercise the same broker rules.
Internal notifications may use a watch channel, but runtime updates never
notify the disk-generation watch or advance inspection cursors or revisions.

Retain `GET /api/v1/runtime/capabilities` and
`POST /api/v1/runtime/page-buffer/observe`, with `Cache-Control: no-store`.
Keep runtime admission separate from both ordinary inspection and disk-watch
admission. Runtime results use their own envelope, not inspection outcomes,
diagnostics, or coverage. The browser sends a bounded ordered VPID scope and
request epoch; the response echoes the accepted scope and provides normalized
states, capture identity, scan interval, limitations, producer completeness,
and separate requested/evaluated counts. The existing 512-page browser cap is
not a limit on the full producer scan. Explicit resident inspection remains
outside this map, as decided in Choose the consistency-inspection boundary.

### Scheduling and bounded retention

Retain the existing browser defaults of 500 ms for selected-page state and
2 s for visible-page state, subject to the resource-budget decision. These
are consumer schedules, not point operations on the producer. Reuse a
sufficiently recent scan or join one in-flight refresh. Do not create parallel
producer scans, a per-tab producer connection, or a missed-tick backlog.

Hidden tabs stop runtime requests. Paused tabs poll capability metadata only;
metadata polling does not cause scans. Another active tab may refresh the
shared cache, allowing paused tabs to report newer evidence availability
without adopting it. Resume requests fresh evidence. Retain only bounded
current evidence and the latest offer, not a runtime event history.

Browser scope overflow is explicit and rotates the non-selected portion as
specified by the existing consumer contract. Browser admission and producer
scan completeness remain separate: complete browser coverage cannot turn a
truncated producer scan into a complete one. Only omission from a complete
scan supports not-resident; missing partial-scan records and unevaluated pages
are unknown. Never merge different scans into one apparent capture.

### Lifecycle and adoption

Apply the accepted peer, identity, loopback, and explicit-attachment policy
from Set the security posture of an engine-connected serve.

- Transient timeout/disconnect discards unfinished scan assembly, retains the
  previous usable scan with its original age, and retries using capped
  exponential backoff with jitter. Reset backoff after successful recovery.
  Distinguish retained aged evidence from a currently working connection.
- Peer/identity refusal or protocol incompatibility clears producer evidence
  and requires explicit retry. Disabled or unavailable sources with no usable
  retained evidence show no overlay and a neutral capability explanation.
- A new server incarnation clears all previous runtime data and revokes
  in-flight adoption authority, even while paused. Revalidate the handshake
  before accepting new records. Pause does not preserve invalid identities.
- Route, requested scope, overlay, and pause changes revoke incompatible
  browser response authority. Cancellation helps save work; scope/epoch checks
  remain the authority. One tab cancelling cannot invalidate another tab's
  demand for the shared scan.
- A disk-generation change alone may retain state-only evidence for the same
  proven database/volume identity. It never establishes page-image
  correspondence. Identity changes revoke that evidence.

Display capture age and scan interval separately from disk reading time.
Cache hits and HTTP response times must not reset source age. Do not claim
that the scan records, disk facts, and displayed page were captured at one
instant. Exact clock/age calculations and retry/deadline ceilings are part of
the resource-budget decision below.

### Resource-budget resolution

[Set overlay resource budgets and measurement gates](15-set-overlay-resource-budgets.md)
now settles producer scan record/byte/deadline caps, bounded cache and HTTP
admission, retry ceilings, age calculations, expiry even while paused, and
measurable acceptance gates. Its resolution is authoritative for these
details; no measurement-backed performance claim is made here.
[Define the cross-repo verification strategy](11-define-verification-strategy.md)
now assigns test layers, owners, venues and evidence requirements. The final
cross-repo handoff remains open.
