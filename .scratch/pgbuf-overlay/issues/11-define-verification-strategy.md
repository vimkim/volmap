Type: grilling
Status: resolved
Blocked by: 05, 10, 15

# Define the cross-repo verification strategy

## Question

How is the overlay proven correct on both sides without requiring a live engine in every test run?

1. A fake inspector endpoint speaking wire v1 (the design reference's in-memory adapter) as the primary volmap test double — scripted residency/dirty scenarios, version mismatches, refusals, incarnation changes.
2. Web-only semantic rendering fixtures for the selected state-marks and LRU-topology layers at both Volume and Sector scale, including fresh, stale, paused, and absent capability states. Prefer semantic-cell assertions over screenshot-pixel coupling; the runtime overlay is excluded from TUI parity by ticket 07.
3. The real-engine gate — a debug-build `cub_server` + `demodb` integration check exercising the actual socket: where it runs (local release gate vs CI), and what minimal assertions it makes (residency of a just-fixed page, dirty after update, gone after eviction).
4. CUBRID-side tests — what the upstream PR carries (unit tests for the scan/serializer; whether a system-parameter-gated feature gets a shell test).
5. Wire-contract conformance — one shared description both repos test against, so contract drift is caught by tests, not integration.
6. Visual resource and accessibility gates — the 10,000-page mosaic stays responsive without animation, every state has a non-color carrier, and exact LRU-list indices remain reachable in selected-Page detail.

## Comments

- The accepted architecture adds shared-scan coalescing, partial-versus-broken
  scan handling, independent browser/producer coverage, and scan-interval
  timing. Verify these through the broker's production and simulated source
  adapters, including multiple tabs and incarnation changes while paused.
- [Set overlay resource budgets and measurement gates](15-set-overlay-resource-budgets.md)
  is resolved and owns the numerical limits, workload matrix and performance
  gates. This ticket assigns conformance fixtures, harnesses, execution
  venues and evidence artifacts without silently weakening those gates.

## Answer

Resolved with the user through two rounds of “all recommended”. This records
test ownership, execution venues, required assertions and evidence. It does
not implement tests or claim any gate has passed.

### Infrastructure evidence (read-only survey, 2026-09-08)

- Volmap has Rust tests (`justfile:33`, `release/check.sh:152`) and a real
  HTTP fixture server (`src/web.rs:2612`). No runtime UDS fake/client test
  harness was found in `src/` or `tests/`; it would be new work.
- Browser model/effect tests exist in `web/src/model.test.ts` and
  `web/src/runtime.test.ts`. Playwright config runs Chromium and Firefox;
  `release/run-browser-server.sh:21` starts debug Volmap with synthetic disk
  fixtures and `--no-follow`, not CUBRID. Existing browser cases and semantic
  accessibility checks do not establish overlay density/accessibility gates.
- Volmap has local/release verification scripts, but no `.github` directory
  was found. Do not describe proposed gates as existing hosted CI jobs.
- CUBRID at `/home/vimkim/gh/cb/pgbuf-bcb-report` pins Catch2 2.11.3 in
  `unit_tests/CMakeLists.txt:46`; packing and JSON-builder targets offer
  unit-test patterns. No overlay serializer target exists. No actual ctest
  registration was found in the inspected root/unit-test CMake wiring, so
  test execution must be proven rather than inferred from AGENTS guidance.
- CUBRID CircleCI consumes external testcase repositories; shell uses an
  external private repository and a debug build. No local `tests/` directory
  exists in this engine worktree. Current infrastructure does not establish
  a feature-specific release-plus-debug integration matrix.
- These are infrastructure facts, not test results. No suite was executed.

Both rounds below are accepted. No test implementation or gate execution is
part of this ticket.

### Accepted first round

- Everyday deterministic layers: CUBRID scan/serializer tests, Volmap against
  a scripted Unix-socket producer, and browser tests against normalized HTTP.
  Required real-engine integration and performance gates supplement them.
- Versioned wire-conformance corpus owned beside the CUBRID producer;
  Volmap vendors an exact revision/hash-pinned copy. Include valid, partial,
  malformed, additive-field, limit-boundary and incarnation scenarios with
  expected semantics. Both repos test offline independently. Corpus changes
  require matching cross-repo evidence before delivery; no new shared runtime
  dependency is implied.
- Semantic-cell/accessibility assertions at Volume and Sector scales and a
  controlled 10,000-page density case. Cover state marks, LRU mode,
  stale/paused/expired/absent states, keyboard access, non-color labels,
  reduced motion and high contrast. A small screenshot set supports visual
  review rather than replacing semantic assertions. No runtime TUI parity.

### Accepted second round

- Ownership/venues: Volmap owns offline corpus, scripted UDS, broker and
  browser checks in local/release scripts. The CUBRID PR carries scan and
  serializer unit tests with a companion shell case in the external testcase
  repository. Prove test binaries actually execute; do not infer execution
  from a successful build or an empty ctest run.
- Before cross-repo delivery, require real-socket integration on debug and
  release builds: producer checks on develop, full Volmap integration on the
  agreed format-aligned branch. Run the accepted performance matrix on a
  pinned dedicated host using release builds and attach exact-commit
  artifacts. Hosted CI may automate these gates later; absent CI or private
  testcase access never means passing.
- Engine oracles: controlled test-only fixtures establish known VPIDs and
  held residency/dirty/eviction conditions with bounded synchronization, not
  sleeps. The observer remains non-invasive. A fixture unable to establish
  its precondition is inconclusive, not passing. No production control
  endpoint is added. Real-socket cases also cover parameter-off absence,
  successful attachment, complete/partial framing, disconnect/restart,
  peer/identity refusal, and unaffected ordinary inspection.
- Evidence: one gate manifest maps every accepted invariant and budget to
  its owning test, exact commits/corpus hash, environment, executed test
  count, result and raw artifacts. Missing/skipped/inconclusive required
  checks block delivery.
- Browser: the controlled 10,000-page case requires p95 input-to-visible
  update at most 100 ms on the recorded reference host in Chromium and
  Firefox, alongside accepted memory gates. Test actual rendered density,
  keyboard navigation, high contrast and reduced motion. Manual screen-reader
  and visual review evidence supplements automation; screenshots alone
  cannot establish accessibility.

### Required coverage and owning harnesses

| Layer / owner | Required proof | Execution and evidence |
| --- | --- | --- |
| CUBRID producer unit tests | Semantic page-kind mapping on both branches; coherent latch tuple and LRU tuple decoding; bounded traversal/rotation, serialization, footer reservation, exact counts and limit boundaries | Engine unit-test executables following existing Catch2 patterns; named executed cases and nonzero counts, not build success alone |
| Shared conformance corpus / CUBRID authority, both consumers | Version/framing, complete and truncated scans, unknown optional fields, missing mandatory framing fields, invalid JSON/depth, oversize frames, wrong count/sequence/incarnation, duplicate VPID ambiguity | Versioned expected outcomes and exact content hash; CUBRID encoder and Volmap decoder assertions independently, with offline vendored equality checked before delivery |
| Volmap scripted UDS and broker tests | Chunked/coalesced stream reads, missing footer/EOF/stall, retained evidence on transient failure, refusal clearing, peer/identity checks, one connection/scan across callers, admission/cancellation, bounded allocation | Rust integration tests using the real parser/UDS adapter and broker interface; fake clock/scheduler for deterministic deadlines and retry |
| Volmap HTTP and model/effect tests | No-store/loopback policy, sanitized disclosure, separate runtime admission, exact scope/epoch echoes, selected-first rotation, unknown partial gaps, no cross-scan merging, stale/expiry and pause/restart authority | Extend existing HTTP fixture server and browser model/effect tests; assert disk outcomes, generations and diagnostics remain independent |
| Browser / Volmap | State marks preserve storage colors; LRU alternative with exact indices in detail; fresh/stale/paused/expired/absent and refusal states; Volume/Sector scale and 10,000-page density | Chromium and Firefox Playwright, semantic assertions, timed input-to-visible updates and memory evidence; supporting screenshots plus manual keyboard/screen-reader/visual review |
| Real engine / CUBRID plus cross-repo integration owner | Actual socket/gating/security/lifecycle and controlled residency, dirty and eviction assertions | Debug and release producer checks on develop; debug and release full integration on the format-aligned branch with disposable database fixtures; companion external shell case and exact build/test revisions |
| Performance / cross-repo integration owner | All accepted producer/broker/browser memory, CPU, throughput, latency, completeness and interference gates | Dedicated recorded host, release builds, full workload/pool/tab matrix and paired-run protocol from the budget decision; raw samples and confidence calculations |

The final handoff assigns concrete testcase paths and invocation commands to
the implementation owners. It must not claim hosted CI exists, use personal
convenience commands as CUBRID organization workflow, or treat unavailable
external test infrastructure as a skipped passing gate.

### Assertions that must not become timing guesses

- Establish fixture preconditions independently of the inspector response.
  Known-resident/dirty cases hold the required condition across observation;
  eviction cases prove removal and prevent refixing until a complete scan
  finishes. Never infer absence from partial coverage. Test control belongs
  to fixture setup, not the observer, and must not require a shipped control
  endpoint or alter the inspected state as a side effect of observation.
- The real-socket gate uses the shipped producer path. Test-only fixture
  machinery cannot substitute for verifying unmodified release attachment,
  gating, serialization and lifecycle. If a controlled state precondition
  cannot be established, report that case as inconclusive and keep its gate
  open rather than replacing it with a sleep or a weaker assertion.
- Exercise both parameter settings, private socket permissions, exact-UID
  refusal, identity mismatch, stale-socket handling and activation failure
  that leaves the database running. Credential-isolation cases require a
  suitable isolated environment; lack of that environment is missing evidence.
- Coverage and temporal tests include complete omission versus partial
  unknown, duplicate ambiguity, malformed capture discard, concurrent waiters,
  last-observer cancellation, overload, cached response age, clock steps,
  suspended/hidden tabs, paused expiry and incarnation changes while paused.
  Validate the production adapter and the deterministic model at their
  respective seams; simulated time alone does not prove real I/O timeouts.
- Resource tests exercise below/at/above each accepted cap, including retained
  plus in-flight scans and concurrent serialized responses. Assert progress
  under stable-pool rotation, explicit overload and continued disk inspection.
- Browser fixtures must distinguish total modeled pages from actually
  rendered cells. Record viewport, rendered density, browser version and
  interaction samples; an off-screen 10,000-page array is not density proof.
  Include selected-Page detail and keyboard focus after viewport/scope changes.
  Static state marks remain animation-free and all states have non-color
  carriers. Use the budget decision's recorded-run discipline for quantitative
  evidence; report each browser rather than averaging away a failure.

### Delivery evidence and enforcement

The gate manifest is produced during implementation/verification, not in this
planning session. It records requirement, test owner/path, exact producer and
consumer commits, corpus revision/hash, build mode, environment, fixture
preconditions, command, executed counts, pass/fail/inconclusive status and raw
artifact links. Performance entries also record the budget decision, workload,
samples, baseline and confidence method; manual review entries record reviewer,
browser/assistive technology and findings. Sanitized logs must not leak paths,
credentials or application page contents through user-facing runtime output.

Every required row must have current evidence before cross-repo delivery.
Missing, skipped, inconclusive or failing evidence leaves the gate open. A
code/corpus change invalidates affected evidence and requires rerunning those
checks; do not carry a passing label across unrelated commits without proving
the evidence still covers the delivered change. The accepted budget thresholds
remain authoritative; this decision adds the browser responsiveness gate but
does not weaken or replace the existing performance matrix.

AOUT history is excluded by
[Decide whether AOUT history belongs in this overlay](16-decide-aout-overlay-scope.md);
no AOUT-specific verification gate is required for this handoff.
Flush-transition events are also excluded by
[Decide whether flush-transition events belong in this overlay](17-decide-transition-changefeed-scope.md).
No event-capture/replay gate is required; sampled flushing-state coverage and
the existing limits remain required. Tests must not reinterpret scan changes
as proof of event timing, counts, cause or durability.
