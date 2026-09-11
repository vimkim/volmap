Type: grilling
Status: resolved
Blocked by: 05, 10

# Set overlay resource budgets and measurement gates

## Question

Given the shared, demand-driven broker chosen in
[Define the volmap overlay architecture](10-define-volmap-overlay-architecture.md),
set concrete limits and acceptance gates before implementation handoff:

1. Producer traversal, emitted-record, frame/total-byte and elapsed-time caps;
   client validation of footer/count/truncation; bounded handling of malformed
   or over-budget streams without publishing an unfinished capture.
2. Broker peak memory, including the retained scan and in-flight assembly;
   resident-pool scaling, cache eviction/admission behavior, and how a pool
   larger than the cap remains explicitly partial. A 512-page HTTP scope does
   not bound producer scan memory or cost.
3. HTTP request size, response bytes, concurrent requests, waiting callers,
   cancellation and timeout limits. How does fair coalescing behave across
   tabs with different cadences without starving disk inspection?
4. Validate or revise 500 ms selected-page and 2 s visible-page defaults;
   choose freshness/cache reuse rules, connection/scan deadlines, exponential
   retry base/ceiling/jitter, and paused capability-check cadence. Respect the
   accepted producer 100 ms minimum interval and two-client limit.
5. Define capture-age calculations from scan intervals, handling wall-clock
   steps or uncertain source clock alignment without inventing per-page time
   or renewing age on cache reads. Set any maximum retained-evidence age.
6. Choose representative pool sizes/workloads and latency, CPU and memory
   thresholds, separating decisions needed now from measurements required as
   implementation/release gates. Include full/partial scans, stalled producer,
   many tabs, viewport rotation, pause/hidden tabs and restart.

## Comments

Created from the user's accepted architecture recommendation on 2026-09-08.
This graduates the map's polling/scan-cost/performance-budget fog and blocks
verification strategy and the implementation handoff.

## Answer

Resolved with the user through three rounds of “all recommended”. These
are accepted implementation limits and future measurement gates, not
measured performance claims. Production implementation and gate execution
remain outside this planning map.

### Baseline evidence (read-only, 2026-09-08)

- CUBRID worktree `/home/vimkim/gh/cb/pgbuf-bcb-report`:
  `src/base/system_parameter.c:1201` defaults to 32,768 buffer pages;
  `src/storage/storage_common.h:91` defaults to 16 KiB pages. Their product
  is 512 MiB configured capacity, not measured RSS. The proposed 1 GiB
  reference therefore covers twice that default slot count.
- Volmap `src/cli.rs:31` supplies 256 MiB disk-inspection memory and 2 GiB
  spill defaults. These are not runtime-broker budgets and must not be
  borrowed without accounting for simultaneous disk inspection.
- Volmap `src/web.rs:42` currently bounds JSON bodies at 64 KiB, ordinary
  concurrent requests at 32, and watch requests separately at 64. Runtime
  admission remains a separate design decision.
- `tests/resource_benchmark.rs:390` reports disk-scan latency distributions,
  RSS/peak RSS, admitted memory and spill bytes; `justfile:119` selects 30
  samples for the local full benchmark. This existing harness does not
  measure producer overhead or browser overlay performance. No overlay
  benchmark was executed, and none of the proposed thresholds is a measured
  result.

The user accepted both first-round recommendations with “all recommended”.
The dependent limits and final measurement recommendations below were also
accepted by the user.

### Accepted first round (2026-09-08)

- Coverage: target complete scans of a 1 GiB pool at 16 KiB/page (65,536
  slots) under reference conditions, with fixed caps. Any cap can yield an
  explicitly partial scan, including on smaller pools under load. Larger
  pools remain best-effort, without automatic resource expansion.
- Measurement policy: provisional design limits, not measured performance
  claims. Accepted release gates are at most 2% throughput loss and 5% p99
  transaction-latency increase in paired active-observation workload tests;
  measure parameter-off overhead separately. A failed gate requires tuning
  or an explicit design revision, not a silent waiver.

The following rounds specify the dependent limits and measurement protocol.

### Accepted second round

The user accepted all four recommendations with “all recommended”.

- Producer: 65,536 visited slots and emitted records maximum per scan;
  64 MiB scan bytes including framing/footer; 4 KiB record frames; 100 ms
  traversal/serialization deadline checked between slots (not a real-time
  guarantee). Bound output buffering to 64 KiB, disconnect after 250 ms
  without write progress, and enforce a 2 s whole-exchange deadline. Reserve
  footer capacity; a valid footer is mandatory for publishing partial scans.
  Rotate starting slots after truncation without merging captures. Handshake
  framing and validation are specified in the final round below.
- Broker/HTTP: 128 MiB total accounted overlay memory, including retained and
  in-flight scans, indexes, parser buffers and responses; 48 MiB maximum per
  decoded scan; no spill/history. Client assembly overflow discards unfinished
  evidence, retaining the previous usable scan. Requests admit at most 512
  VPIDs / 64 KiB; responses at most 1 MiB. Eight admitted observation requests
  including waiters, no extra queue, 2.5 s deadline, excess gets HTTP 429.
  Runtime admission is separate from disk inspection. Accounting ceilings
  are not claims that process RSS equals those ceilings.
- Scheduling: selected 500 ms / visible 2 s; broker scan-start floor 500 ms;
  cache reuse only within caller cadence; coalescing without missed-tick
  queues. Transient retry delays 0.5/1/2/4/8 s then 8 s, with +/-20% jitter.
  Paused metadata checks every 5 s; hidden tabs send nothing. Producer keeps
  its separate 100 ms floor. The final round distinguishes the nominal retry
  ceiling from the jittered maximum.
- Age: conservative upper bound from broker monotonic request start,
  preserved across cache reads, plus browser elapsed time and HTTP round-trip
  uncertainty. Source wall-clock times are display metadata, not freshness
  authority. Fresh requires upper-bound age within two requested intervals;
  uncertain age never counts as fresh. Expire evidence after 30 s even when
  paused, leaving an expired explanation.

### Accepted final round

The user accepted both recommendations with “all recommended”.

Protocol/lifecycle recommendation:

- Handshake at most 64 KiB; connect plus handshake deadline 500 ms. An
  oversized identity set refuses attachment rather than weakening identity
  proof. Record-frame limit remains 4 KiB, including newline; JSON nesting
  depth at most 16. Bound control frames to 4 KiB as well.
- Require valid framing, matching sequence/incarnation, an exact footer
  record count, and explicit truncation. Discard malformed captures. Count
  received records before deduplication; repeated VPIDs become ambiguous and
  unknown rather than last-write-wins. Never infer absence from malformed,
  incomplete, truncated or unevaluated evidence.
- Retry ceiling is nominally 8 s, at most 9.6 s after jitter; reset only after
  a valid scan. Capability requests have four separate admission slots, a
  1 s deadline and no queue. Last-observer cancellation stops unnecessary
  refresh work; it cannot cancel another observer's demand. Resume requires
  a scan started after resume, respecting the 500 ms floor.

Measurement recommendation (all are future gates, not results):

- Pools: 512 MiB, 1 GiB and 4 GiB at 16 KiB/page. Workloads: idle,
  read-heavy, write/flush-heavy and churn. Exercise 1/8/32 tabs, producer
  stalls, viewport rotation, pause/hidden, clock changes and restart.
- Idle/read-heavy 1 GiB reference: at least 99% complete scans, p95 refresh
  at most 250 ms. Cached HTTP p95 at most 25 ms; simultaneous disk-inspection
  p95 regression at most 5%. Oversized pools require truthful partial
  coverage, not the reference completeness gate.
- At default cadence: producer CPU at most 20% and broker CPU at most 50%
  of one logical core. Incremental peak RSS limits: producer 16 MiB, broker
  192 MiB, each browser tab 32 MiB. Keep the independent 128 MiB accounted
  broker-allocation ceiling; RSS allowance does not authorize exceeding it.
- Ten paired runs per performance case; each has 60 s warmup and at least
  5 min measurement, extending to at least 100,000 transactions where
  applicable. The 95% confidence upper bound must meet the accepted
  throughput/transaction-latency overhead limits; uncertain results are
  inconclusive, not passing. Record hardware, software commits, build mode,
  dataset, concurrency, seeds, raw samples and confidence calculation.
- All resource-cap, malformed-input and lifecycle tests must pass. Disabled
  producer behavior is measured separately. Tests must exercise retained
  evidence plus in-flight assembly and concurrent responses together, not
  just measure an empty cache. No benchmark has been executed in this map.

### Interpretation and handoff checks

- All byte limits use binary units and include framing. Enforce limits before
  unbounded allocation: do not buffer a whole 64 MiB stream merely to parse
  it. Charge allocated capacities, indexes, duplicate tracking and concurrent
  response serialization against the broker total. Retained scans held by
  outstanding requests still count after cache replacement. Admission may
  stop earlier than a per-object ceiling when the total would be exceeded.
- Reserve footer space before admitting the next record. A limit reached
  exactly at complete traversal need not imply truncation; stopping before
  visiting the entire pool does. Each scan visits a slot at most once.
  Producer rotation advances beyond the visited span; no multi-scan union
  can establish absence. Stable-pool rotation tests must prove progress.
- A client-side limit violation cannot manufacture a valid partial footer.
  Discard that assembly and retain only the prior usable, unexpired capture.
  Unknown optional fields remain additive-compatible within framing limits;
  mandatory framing/identity/count fields are not optional page-state fields.
- The traversal deadline starts with traversal, includes elapsed backpressure
  time, and is checked between slots. The whole-exchange deadline includes
  draining/footer transmission. No deadline authorizes blocking a database
  worker or page latch for its duration. Scheduler delays are measured,
  not represented as a hard real-time guarantee.
- Per-caller freshness uses its requested cadence, not another tab's faster
  cadence. All admitted waiters can consume the same refreshed capture, with
  independent scope/epoch validation. Admission is bounded, not a promise of
  starvation-free service to arbitrarily many clients; overload tests must
  show explicit rejection and preserved disk-inspection service.
- Let S be broker monotonic scan-request start and E be monotonic response
  serialization time. Send upper-bound age E-S, never a new capture time.
  At browser receipt, conservatively add the browser-measured full HTTP
  round trip; afterward add browser monotonic elapsed time. This deliberately
  overestimates transport age. If suspension or clock behavior makes elapsed
  time untrustworthy, revoke freshness and require fresh evidence rather
  than extending retention. Source wall-clock steps never renew freshness.
  Expiry applies to browser and broker evidence, even paused; capability
  metadata is not retained page evidence.
- Reference refresh latency is measured from refresh demand to published
  validated capture; cached HTTP latency is request to completed response.
  CPU percentages are CPU-time/wall-time relative to one logical core;
  incremental RSS compares matched source-disabled runs at the same
  workload/tab count. Report allocator-accounting peaks separately from RSS.
  Reference completeness must be measured independently of latency so that
  fast, mostly empty partial results cannot satisfy the usefulness gate.
- Paired transaction comparisons use identical workload, database state,
  concurrency and build except inspector enablement/activity. Include an
  unmodified-build versus parameter-off comparison and enabled/no-demand
  behavior in the separate disabled/idle evidence. Record confidence method
  and per-case results; do not average away a failing case. The verification
  strategy assigns harnesses and execution venues, not new budget values.

The accepted thresholds are the initial contract. Failing measurements leave
the implementation/release gate open; tuning within the contract is allowed,
but changing coverage, cadence or limits requires an explicit design revision.
