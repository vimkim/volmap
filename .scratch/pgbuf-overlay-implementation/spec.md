# State-only CUBRID page-buffer observation overlay

Status: ready-for-agent

Scope: Volmap implementation; cooperating CUBRID producer and conformance
corpus supplied through CBRD-27398. Published to the configured local Markdown
tracker. No implementation, release result or external publication is implied.

## Problem Statement

An operator inspecting a CUBRID database with Volmap can see observed disk
state but cannot relate the selected physical page to independently observed
buffer residency, latch/fix state, dirty/flushing state or replacement-list
membership. Aggregate engine monitoring does not provide that page-level
context. Showing such state without explicit identity, age and coverage would
also be misleading: the buffer pool changes during inspection, and neither
matching page identity nor a recent reading proves memory/disk correspondence,
commit visibility or durability.

The operator needs a bounded, optional runtime observation overlay that helps
explain engine state while preserving ordinary read-only inspection, existing
storage colors and accessibility. An absent, overloaded or restarted engine
must not invalidate disk inspection or silently produce authoritative-looking
runtime results.

## Solution

Add an explicitly enabled, loopback-only CUBRID page-buffer observation
capability to Volmap's live web viewer. A private shared broker authenticates
and identifies one cooperating producer, validates bounded resident-set scans,
and supplies observations for the selected page and visible sectors. Browser
requests share scans rather than generating a producer connection per tab.

The overlay preserves observed disk state beneath static residency, dirty and
flushing marks, with a separate LRU-topology mode and detailed selected-page
state. It reports source capability, capture interval, conservative age,
partial coverage and expiry explicitly. Pausing stops adoption, hidden tabs
stop requests, and identity changes revoke stale authority even while paused.
Runtime evidence never becomes an inspection fact, revision, diagnostic,
export or terminal-parity obligation.

## User Stories

1. As an operator, I want ordinary disk inspection to work without CUBRID running, so that runtime observation remains optional.
2. As an operator, I want to enable runtime attachment explicitly and supply its socket path, so that Volmap cannot attach to an unintended producer.
3. As an operator, I want non-loopback runtime attachment rejected at startup, so that unauthenticated engine observations are not exposed on the network.
4. As a remote operator, I want to use SSH forwarding to the loopback viewer, so that remote access does not require a new public inspector transport.
5. As an operator, I want both peers and the complete persistent-volume identity verified, so that a copied or similarly named database cannot supply observations for my inspection.
6. As an operator, I want sanitized capability and refusal explanations, so that I can understand failures without exposing paths or process identities.
7. As an operator, I want the selected page's observed residency, so that I can relate its physical identity to buffer-pool evidence.
8. As an operator, I want observed latch mode, waiter presence and fix count in selected-Page detail, so that I can examine access state without cluttering the dense grid.
9. As an operator, I want dirty, flushing and asynchronous-flush-requested state, so that I can distinguish sampled engine conditions without inventing a flush timeline.
10. As an operator, I want semantic page kinds and log positions, so that branch-specific enum values do not leak into the interface.
11. As an operator, I want exact LRU kind-local membership in selected-Page detail, so that I can identify the observed shared or private list within one server incarnation.
12. As an operator, I want a separate LRU-topology color mode, so that I can examine replacement organization without confusing it with storage allocation colors.
13. As an operator, I want residency outlines, dirty corners and static flushing edges, so that observed disk structure stays visible under runtime marks.
14. As an operator, I want runtime state at both Volume and Sector scales, so that overview and focused navigation use consistent meanings.
15. As an operator, I want the selected page prioritized within bounded requests, so that viewport density does not silently exclude my primary target.
16. As an operator, I want non-selected coverage to rotate explicitly when the viewport exceeds the request budget, so that sampling does not permanently hide the same pages.
17. As an operator, I want requested and evaluated page counts separately, so that I know which parts of my scope were actually considered.
18. As an operator, I want producer completeness distinguished from browser scope coverage, so that a fully evaluated request cannot disguise a truncated producer scan.
19. As an operator, I want missing pages in partial scans labeled unknown, so that missing evidence is not shown as confirmed nonresidency.
20. As an operator, I want ambiguous duplicate page observations identified, so that a non-atomic scan cannot silently select an arbitrary last value.
21. As an operator, I want the original capture interval and conservative observation age, so that cached responses do not appear newly captured.
22. As an operator, I want freshness relative to my sampling cadence, so that another tab's schedule does not change the meaning of my evidence.
23. As an operator, I want uncertain or expired evidence clearly distinguished from fresh evidence, so that clock changes and long pauses cannot extend its apparent validity.
24. As an operator, I want selected-page and visible-page observation at their accepted default cadences, so that useful updates have predictable costs.
25. As an operator using several tabs, I want requests to share one broker connection and in-flight scan, so that adding tabs does not multiply producer traversal work.
26. As an operator, I want explicit overload responses and bounded waiting, so that observation traffic cannot starve ordinary disk inspection.
27. As an operator, I want a transient broken stream to preserve only prior usable, unexpired evidence with its original age, so that failures do not masquerade as new partial captures.
28. As an operator, I want peer refusal, identity mismatch and protocol incompatibility to clear evidence and require explicit retry, so that unsafe attachment cannot recover silently.
29. As an operator, I want pause to freeze adoption of both disk generations and runtime observations, so that I can inspect a stable presentation without preserving expired or invalid identities.
30. As an operator, I want paused capability checks to report newer evidence availability without triggering scans, so that pause does not consume hidden producer work.
31. As an operator, I want hidden tabs to send no runtime requests, so that background pages do not consume observation resources.
32. As an operator, I want resume to request evidence captured after resuming, so that an old cache hit does not look like recovery.
33. As an operator, I want route, scope, overlay and pause changes to reject incompatible late responses, so that observations cannot land on the wrong selection.
34. As an operator, I want a producer restart to clear all old-incarnation evidence even while paused, so that retained state is never attributed to the new server.
35. As an operator, I want ordinary disk-generation changes handled independently of runtime identity, so that they neither fabricate page correspondence nor unnecessarily erase compatible retained evidence.
36. As a keyboard user, I want to reach overlay controls, page details and limitations with stable focus, so that live updates do not interrupt navigation.
37. As a user who cannot rely on color, I want glyphs, text and accessible names for every runtime state, so that the overlay remains understandable in high contrast.
38. As a user with reduced-motion preferences, I want static state marks and quiet age updates, so that polling does not introduce distracting animation or repeated announcements.
39. As an operator viewing a dense volume, I want responsive interaction at the accepted 10,000-page display density, so that runtime observation does not make navigation impractical.
40. As a maintainer, I want deterministic broker, protocol and browser tests without a live engine in every run, so that lifecycle regressions have a fast reproducible test surface.
41. As a maintainer of either repository, I want the same revision/hash-pinned conformance corpus, so that wire drift is detected independently and offline.
42. As a release reviewer, I want real-engine debug/release checks and exact-commit performance evidence, so that simulated success cannot stand in for production behavior.
43. As a release reviewer, I want missing, skipped and inconclusive required checks to remain non-passing, so that incomplete evidence cannot authorize delivery.
44. As an operator, I want documentation to distinguish observations from inspection facts and event history, so that I do not infer commits, durability or flush causality from state marks.

## Implementation Decisions

### Modules and interfaces

1. Add one deep runtime-observation module per attached live inspection
   session, owned independently from the disk-follow module. Its small
   interface provides capability metadata and bounded observations for a
   validated scope. It hides peer/identity verification, wire framing,
   connection lifecycle, assembly, cache replacement, coalescing, retries
   and resource accounting from HTTP handlers and the browser.
2. Use a private producer-adapter seam with real Unix-socket and deterministic
   simulated adapters. Inject clock/scheduling dependencies. Reuse existing
   browser model/effect and HTTP interfaces; do not distribute producer
   protocol knowledge across callers or create a public interface per helper.
3. Runtime publication does not advance disk snapshot generations,
   inspection revisions or cursors, and does not notify disk-watch clients.
   Keep runtime capability, coverage, errors and admission separate from
   inspection outcomes, diagnostics and resource queues.
4. Preserve the standalone executable contract: no required CUBRID library,
   installation or service dependency for ordinary inspection. Use existing
   JSON dependencies; optional runtime sources retain independent contracts.
5. Update serve configuration, browser state/effects, runtime controls,
   selected-Page detail, heatmap rendering and user documentation. Preserve
   the accepted domain glossary and loopback/web-only ADR. No inspection-graph
   schema or export-format change is required.

### Attachment and security

6. Runtime attachment requires explicit enablement and an explicit producer
   socket path. No discovery or environment fallback is allowed. Requested
   attachment with a non-loopback HTTP listener is a startup error, not a
   warning or downgrade. Unattached serving keeps its existing behavior;
   remote runtime use goes through SSH forwarding, without incidental HTTP
   authentication or TLS implementation.
7. Authenticate connected peers through Linux SO_PEERCRED. Effective UIDs
   must agree exactly; Volmap also checks private-directory and socket
   ownership. There is no root or group exception. Peer refusal occurs before
   accepting protocol data as trusted observation evidence.
8. Verify database-creation identity and the complete persistent-volume set
   using each volume's volid, volume-creation identity, device and inode.
   Temporary volumes do not participate in this set. Names/paths alone are
   insufficient. Bind observations to an unpredictable server incarnation;
   identity mismatch maps to refused with stable reason identity-mismatch.
   If the complete identity set exceeds the handshake cap, refuse attachment
   rather than weakening identity proof or accepting a subset.
9. The cooperating producer uses its existing Unix-socket namespace with
   a dedicated owner-only directory and socket modes 0700 and 0600. Unsafe
   symlinks, wrong owners, non-sockets and active sockets are preserved.
   Reclaim only a confirmed stale same-owner socket; shutdown removes only
   the exact socket created. Activation failure leaves the inspector
   unavailable for that incarnation without stopping database startup; no
   background bind retry is permitted.
10. Disclose sanitized capability state, verification result, protocol
    version, database fingerprint, abbreviated incarnation, capture times
    and stable limitations/reasons. Do not expose paths, UID/GID/PID, raw OS
    errors, pointers, private structures or application page contents.
    Name the source CUBRID page-buffer observation, not a live database.
    Preserve capability states disabled, connecting, active, stale,
    unavailable, refused and incompatible. Pause, expiry and coverage are
    independent presentation/evidence conditions, not inspection outcomes.

### Producer contract consumed by Volmap

11. The CBRD-27398 producer is an external implementation dependency, not
    CUBRID engine work to perform in Volmap tickets. It provides versioned
    JSON-lines over AF_UNIX SOCK_STREAM, bulk resident-set scans only. No
    point inspection operation, missing-page load, page copy, hash, disk
    read or unbounded page-protection wait is part of this source.
12. Consume semantic page identity and page kind; latch mode
    none/read/write/flush, waiter presence and fix count from one coherent
    atomic latch-word sample; dirty, flushing, asynchronous-flush-requested
    and to-vacuum state; page LSA and oldest-unflush LSA; and LRU zone,
    list kind and kind-local index. LRU zones are lru1/lru2/lru3/void/invalid;
    list kinds are shared/private/none/invalid with null index when not
    applicable. The LRU tuple comes from one flags sample. Neither tuple
    makes the record or pool an atomic snapshot.
13. Handshake carries protocol, verified database/volume identity,
    incarnation and shared/private LRU topology counts. LRU indices are
    incarnation-local. Normalize producer values before HTTP/browser use;
    do not leak raw page-type ordinals or packed flags. Observed per-list
    summaries may be derived from scan records and remain explicitly partial
    when their source is partial; native counters/quotas are excluded.
14. Each scan has a sequence/start header, records, and end/count/truncation
    footer. Sequence increases within an incarnation and is not an event
    sequence. Expose the scan interval, not invented per-page timestamps.
    Field names are snake_case, enums are semantic lowercase strings, and
    same-major evolution is additive. Unknown optional fields are ignored
    within resource limits; missing mandatory identity/framing/count fields
    invalidate the capture rather than becoming optional state.
15. Validate framing, identity/sequence, raw record count and explicit
    truncation before publication. Duplicate VPIDs become ambiguous/unknown,
    never last-write-wins. Only an evaluated requested page omitted from a
    complete scan supports observed nonresidency; partial gaps and
    unevaluated scope are unknown. Observation is not a guarantee of current
    absence. Never combine different scans into an apparent complete capture.
16. Valid truncated scans publish partial evidence. A missing footer,
    malformed stream or client assembly overflow discards the entire
    unfinished assembly and preserves only the previous usable, unexpired
    capture. A client cannot manufacture a footer to salvage a prefix.
17. The producer's startup-only hidden Boolean enable_pgbuf_inspector defaults
    to false, is server-only, and creates neither daemon nor socket when
    disabled. No reloadable, user-change or client-synchronization flags are
    added. The state-only endpoint is supported in
    Unix server release/debug builds, not Windows or non-server binaries;
    there is no new v1 compile option. Producer refusal codes include
    version-unsupported, busy, rate-limited and incarnation-changed.
    Parameter-off may be accepted defensively, but socket absence means the
    v1 producer cannot normally emit it; do not infer the remote setting
    solely from an absent socket.

### HTTP, scope and scheduling

18. Retain GET `/api/v1/runtime/capabilities` and POST
    `/api/v1/runtime/page-buffer/observe`, both Cache-Control: no-store.
    The runtime envelope echoes accepted ordered VPIDs and request/scope
    epoch and carries normalized states, capture identity/interval,
    limitations, producer completeness and separate requested/evaluated
    counts. A 512-page HTTP scope does not limit producer scan work.
19. Build scope selected-page first, then nearest visible-sector pages and
    physical order, with explicit rotation of the non-selected remainder.
    Report any reduced admission or rotation; never sample silently. Do not
    present browser coverage as proof that a producer scan was complete.
20. Share one producer connection, one latest-scan cache and at most one
    in-flight refresh per attached session. A caller uses a capture no older
    than its own cadence or joins refresh demand. Do not create per-tab
    producer scans/connections, missed-tick backlogs or historical caches.
21. Selected-page polling defaults to 500 ms and visible-page polling to
    2 s. The broker starts scans no faster than once per 500 ms, independently
    of the producer's 100 ms floor. Coalescing serves admitted waiters with
    their own scope validation; bounded admission is not a guarantee of
    starvation-free service for arbitrarily many clients.
22. Transient retry nominal delays are 0.5, 1, 2, 4 and 8 s, then 8 s, with
    +/-20% jitter and actual maximum 9.6 s. Reset after a valid scan. Preserve
    aged prior evidence separately from connectivity status. Peer/identity
    refusal or protocol incompatibility clears evidence and requires explicit
    retry rather than automatic reconnection into trusted state.
23. Hidden tabs send no runtime requests. Paused tabs check only metadata
    every 5 s; this does not cause scans or adopt newly available observations.
    Another active tab may refresh the broker and provide a newer-offer
    indication. Resume requires a scan started after resume, respecting the
    scan-start floor. Pause stops adoption of disk generations too, without
    preventing expiry or invalidation.
24. Route, scope, overlay and pause changes revoke incompatible in-flight
    browser adoption authority. Check response scope/epoch even when requests
    are cancelled. A tab's cancellation cannot cancel another's demand; when
    the last observer leaves, stop unnecessary refresh work. Incarnation
    change clears retained evidence and authority even while paused.
25. A disk-generation change prevents incompatible generation-scoped late
    responses from being adopted but does not alone invalidate retained
    state-only evidence for the same proven database/volume identity. No
    generation or VPID match establishes page-image correspondence.

### Age, retention and operational budgets

26. Compute a conservative upper age bound from broker monotonic scan-request
    start to response serialization. Preserve it across cache reads; at the
    browser add its measured full HTTP round trip and subsequent monotonic
    elapsed time. This intentionally overestimates transport age. Source
    wall-clock timestamps are display metadata, not freshness authority.
27. Freshness requires known age or its upper bound within two of the
    requesting caller's expected intervals. Older or uncertain evidence is
    not fresh. Clock steps do not renew age. Untrustworthy elapsed time after
    suspension revokes freshness and requires new evidence rather than
    extending retention. Evict broker and browser evidence after 30 s even
    when paused, leaving an expired explanation rather than a history.
28. Apply the following binary-byte ceilings, including framing, before
    unbounded allocation. These are accepted design limits, not measured
    performance claims.

| Concern | Accepted limit |
| --- | --- |
| Producer scan slots / emitted records | At most 65,536 each; the 1 GiB / 16 KiB-page reference target, not an unlimited completeness guarantee |
| Whole framed scan | 64 MiB, with footer capacity reserved before the next record |
| Record and control frames / handshake | 4 KiB / 64 KiB; JSON nesting at most 16 |
| Producer traversal/serialization | 100 ms elapsed checked between slots, including backpressure time; not a hard real-time guarantee |
| Output buffering / stalled write | 64 KiB / disconnect after 250 ms without progress |
| Connect plus handshake / whole scan exchange | 500 ms / 2 s, with exchange including drain and footer |
| Producer clients / scan floor | At most 2 / 100 ms minimum interval |
| Broker accounted memory / decoded scan | 128 MiB total / 48 MiB per scan; no spill or history |
| Observation request / response | At most 512 VPIDs and 64 KiB body / 1 MiB response |
| Observation admission / deadline | Eight requests including waiters, no extra queue / 2.5 s; overload gets HTTP 429 |
| Capability admission / deadline | Four independent slots, no queue / 1 s |

29. Charge allocated capacities, indexes, duplicate tracking, parser buffers,
    retained and in-flight scans, outstanding response-held scans and
    serialization against the total broker budget. Replacing the latest
    pointer does not uncharge referenced older memory. Total admission may
    stop below an individual object limit. Do not buffer the entire wire
    stream solely to parse it. RSS and accounted allocations are distinct.
30. Producer truncation rotates the next start beyond the visited span, with
    each slot visited at most once per scan. Stopping exactly after full
    traversal need not imply truncation; stopping earlier does. Larger pools
    remain best-effort without automatically expanding resources. Rotation
    does not permit cross-scan absence inference. Deadlines never authorize
    holding database workers or page latches for their full duration.

### Presentation and delivery

31. Preserve storage colors beneath cyan residency outlines, amber dirty
    corners and static magenta flushing edges. Keep LRU-topology colors as
    an alternative mode, exact list indices in selected-Page detail, and
    bursty latch/waiter/fix facts out of dense-grid animation. Show capability,
    freshness, pause, expiry and coverage independently with non-color
    carriers and accessible detail.
32. Preserve keyboard access and compatible focus through refresh/navigation;
    keep age ticks and polling quiet for assistive technology. Respect
    reduced motion and high contrast. Distinguish source-unavailable from
    observed nonresidency; no overlay is preferable to fabricated evidence.
33. CBRD-27398 owns the producer and canonical conformance corpus. Volmap
    vendors an exact revision/hash-pinned copy and tests offline. Concrete
    syntax/corpus generation implements the approved wire semantics; any
    semantic conflict is resolved explicitly before delivery, not hidden in
    browser exceptions or new dependencies.
34. Deliver contract/corpus consolidation, bounded producer and shared
    consumer interface, then visual projection, cross-repo integration/port
    and release readiness. Simulator work need not wait for a live engine;
    real integration must. Producer iteration uses the agreed format-aligned
    baseline e1e651d, followed by a develop port and independent producer checks.
    Do not claim develop disk-format support from semantic wire compatibility.

## Testing Decisions

1. Test observable behavior through the highest useful interface. The shared
   broker's capabilities/observations interface is the main new seam already
   approved during architecture and verification decisions. Exercise its
   production and simulated adapters; avoid tests coupled to private cache
   layout, helper names or exact call sequences unrelated to the contract.
   Test real framing, HTTP disclosure and rendered behavior at their actual
   interfaces rather than replacing them all with broker mocks.
2. Reuse existing Rust unit/integration infrastructure, the real HTTP fixture
   harness, browser model/effect tests and Chromium/Firefox Playwright setup.
   Existing browser tests already assert semantic roles and navigation;
   extend that prior art to runtime behavior. Existing fixtures serve synthetic
   disk volumes, not a live engine. New UDS/corpus/runtime coverage is required.
3. CUBRID owns scan/semantic-mapping/serializer unit tests and the shared
   corpus. Both repositories independently check valid, partial, malformed,
   additive-field, count/identity/sequence, depth/size and duplicate cases
   offline. Corpus changes require matching cross-repo evidence. Actual test
   execution must be shown with named cases and nonzero counts; successful
   compilation or an empty test runner is not a pass.
4. Script socket chunking/coalescing, EOF, missing footer, stalls, bad frames,
   refusal and incarnation changes through the real decoder. Deterministic
   clocks/schedulers cover retry, cache reuse, polling, deadline, pause,
   expiry and cancellation behavior; actual I/O checks separately prove real
   timeout behavior. Test below/at/above every accepted resource cap.
5. Prove independent producer/browser coverage, selected-first rotation,
   complete omission versus unknown partial/unevaluated pages, duplicate
   ambiguity, no cross-scan merge, and retained original age after failures.
   Include stable-pool rotation progress and large-pool partial scans.
6. Exercise simultaneous tabs, retained/in-flight/response allocations,
   shared refresh, last-observer cancellation, overload, differing cadences,
   clock steps/suspension, viewport churn, pause/hidden/resume and restart
   while paused. Assert unaffected ordinary disk inspection, outcomes,
   diagnostics, generations, terminal behavior and exports.
7. Test loopback enforcement, explicit configuration, peer/owner matching,
   copied-volume identity mismatch, sanitized HTTP/UI output and no-store.
   CUBRID real-socket tests additionally prove safe stale-socket handling,
   disabled endpoint absence and optional activation failure. Credential
   isolation requires a suitable environment; its absence is missing evidence.
8. Browser gates cover Volume/Sector semantic cells, state-mark and LRU modes,
   exact-index detail, fresh/stale/paused/expired/absent/refused states, keyboard
   access/focus, non-color labels, high contrast and reduced motion. Use a
   small screenshot set for visual review, not as the sole oracle. Require
   manual screen-reader and visual review evidence; screenshots do not prove
   accessibility.
9. The 10,000-page density case must document actual rendered cells, viewport,
   browser versions and interaction samples, not only an off-screen model
   size. In each of Chromium and Firefox, p95 input-to-visible update must
   be at most 100 ms on the recorded reference host, alongside memory gates.
10. Real-engine integration is a required supplement to deterministic tests.
    Use controlled known-VPID fixtures with independently established,
    bounded synchronization for residency, dirty and eviction conditions;
    avoid sleeps and workload timing guesses. Keep the observer non-invasive
    and add no shipped control endpoint. Failed fixture preconditions are
    inconclusive. Test-only machinery cannot replace checks of the shipped
    producer's unmodified release attachment/lifecycle path.
11. Require develop producer checks and format-aligned full Volmap integration
    separately in both debug and release. CUBRID carries a companion case in
    its external shell testcase repository. Record the actual testcase
    revision; missing private-suite access cannot be silently skipped.
12. Run Volmap deterministic/browser gates through its local/release tooling.
    Hosted CI automation is not currently established and is not invented by
    this specification. Run the accepted performance matrix on a recorded
    dedicated host with release builds; attach exact-commit artifacts before
    cross-repo delivery.
13. Performance cases use 512 MiB, 1 GiB and 4 GiB pools at 16 KiB/page; idle,
    read-heavy, write/flush-heavy and churn workloads; 1/8/32 tabs; and stall,
    rotation, pause/hidden, clock-change and restart scenarios. Test ten paired
    runs per performance case, with 60 s warmup and at least 5 min measurement,
    extending to at least 100,000 transactions where applicable. Preserve
    identical workload/state/concurrency/build between paired comparisons
    except inspector activity. Record raw samples and confidence method.
14. The 95% confidence upper bound must satisfy at most 2% throughput loss
    and 5% transaction p99 increase. Inconclusive results do not pass. Measure
    parameter-off versus an unmodified build and enabled-without-demand
    behavior separately; do not average away a failing case.
15. Require at least 99% complete scans for the idle/read-heavy 1 GiB reference,
    p95 refresh demand-to-validated-publication at most 250 ms, cached HTTP
    request-to-completed-response p95 at most 25 ms, and concurrent disk
    inspection p95 regression at most 5%. Larger pools require truthful
    partial coverage rather than the reference completeness percentage.
    Measure completeness separately so mostly empty fast scans cannot pass.
16. At default cadence, producer CPU is at most 20% and broker CPU at most
    50% of one logical core. Incremental peak RSS limits are producer 16 MiB,
    broker 192 MiB and browser 64 MiB per tab, relative to matched disabled
    workloads/tab counts. Report CPU-time/wall-time and allocator accounting
    separately; RSS allowance does not authorize violating allocation caps.
    Browser allowance follows the [2026-09-11 policy revision](browser-memory-budget-revision.md)
    for ordinary developer PCs with 1–8 tabs; retain the 1/8/32-tab matrix.
17. Maintain a delivery gate manifest mapping every invariant and budget to
    its owner/test, exact producer/consumer commits, corpus hash, build mode,
    environment, testcase revision, preconditions, command, executed count,
    result and raw artifacts. Manual reviews identify reviewer and assistive
    technology. Missing, skipped, inconclusive or failing required evidence
    keeps the gate open; affected evidence must be rerun after code/corpus
    changes. Tuning within the contract is allowed; weakening a threshold
    requires an explicit design revision.

## Out of Scope

- Implementing the CUBRID producer inside Volmap tickets; it is delivered
  through CBRD-27398 with its own approved constraints and shared integration.
- Protected resident-page inspection, page-image copies, digests, DWB or TDE
  comparison, consistency classifications and independent disk-content
  correspondence checks. None is a prerequisite for this overlay.
- AOUT collection, wire fields, history visualization or engine re-enablement.
- Transition event hooks, changefeeds, event buffers/replay and causal
  timelines. Retain sampled flushing fields but do not infer start/end,
  duration, counts, cause or durability from scan differences.
- Runtime observations in TUI, JSON/HTML exports or the inspection graph;
  new terminal-parity channels or canonical navigation history for events.
- New kernel-cache functionality, attribute-selection features or an unrelated
  browser rewrite from the broader runtime project.
- Always-on monitoring, automatic producer discovery, public network inspector
  transport, HTTP authentication/TLS, raw shared-memory/struct exposure,
  stop-the-world snapshots, engine repair/write behavior or hot-path
  instrumentation.
- Claims of commit visibility, currentness, memory/disk equality or durability
  based on VPID identity, sampled state, scan sequence or recency.
- Running builds/tests, changing production code, committing/pushing documents,
  publishing JIRA descriptions or creating PRs as part of this synthesis.

## Further Notes

This specification is the focused Volmap implementation authority synthesized
from the completed planning map. It supersedes broader runtime wording only
for this state-only source; other sources remain independent. The historical
decision tickets preserve rationale, while this specification consolidates
their approved requirements for implementation-ticket creation. Ready-for-agent
means ready to plan/implement under the stated dependencies, not that the
external producer, fixture corpus or release evidence already exists.

The test seams were explicitly accepted in the architecture and verification
rounds; synthesis retains them without adding a new interview. The state-mark
prototype is design evidence only, not production code or measured performance.
No prototype snippet is needed to express this contract more precisely than
the prose and limits above.

References:

- [Reviewed cross-repo handoff](../pgbuf-overlay/handoff/README.md).
- [Completed decision map](../pgbuf-overlay/map.md).
- [Resource budgets and age rules](../pgbuf-overlay/issues/15-set-overlay-resource-budgets.md).
- [Verification strategy and evidence requirements](../pgbuf-overlay/issues/11-define-verification-strategy.md).
- [Delivery order and role ownership](../pgbuf-overlay/handoff/delivery-plan.md).
- [Accepted visual prototype](../pgbuf-overlay/issues/09-prototype-heatmap-encoding.md).
- [Domain glossary](../../CONTEXT.md) and [runtime capability ADR](../../docs/adr/0006-runtime-observations-are-loopback-web-capabilities.md).
- [CBRD-27398 producer issue](http://jira.cubrid.org/browse/CBRD-27398); its
  reviewed local draft remains unpublished by this workflow.

The next workflow is `/to-tickets` using this specification. Create a fresh
implementation ticket graph here rather than reusing the closed Wayfinder
decision tickets or importing excluded dependencies from the broader runtime
plan. Implementation and exact-commit release verification follow separately.
