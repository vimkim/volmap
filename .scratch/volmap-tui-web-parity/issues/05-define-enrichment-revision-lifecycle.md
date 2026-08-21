Label: wayfinder:grilling
Type: grilling
Status: resolved
Assignee: codex
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Define the shared projection boundary for terminal parity](02-define-shared-projection-boundary.md), [Define the TUI navigation, focus, and history state model](03-define-navigation-focus-history-model.md)

# Define automatic enrichment and immutable-revision transitions

## Question

What exact lifecycle begins when a user opens a page whose supported detail has not yet been requested? Specify bounded job admission, visible loading state, cancellation and late completion, success and diagnostic outcomes, explicit adoption of the returned immutable revision, selection/focus restoration, navigation back to older context, input invalidation, and behavior when detail is unsupported, opaque, already complete, or resource-limited. Preserve the web contract's no-silent-mixing rule without importing browser routing into the TUI.

## Answer

### Decision

The user accepted every recommendation over two decision rounds and then confirmed the complete shared understanding. Deepen the accepted `AtlasMachine` with a private, closed enrichment lifecycle; do not add a public job coordinator. The accepted Projection workspace remains the sole owner of semantic eligibility, resource policy, cooperative inspection, exact-head arbitration, immutable publication, and terminal snapshot invalidation. The AtlasMachine owns only presentation-session concerns: whether a request is still relevant, visible progress and notices, cancellation intent, retry suppression, exact revision offers, and transactional adoption.

The terminal host supplies one bounded local worker because `ProjectionWorkspace::enrich` is synchronous and cooperative. That worker is an internal scheduling seam with a production implementation and a deterministic scripted test implementation. Threads, channels, executor handles, Tokio, Crossterm events, HTTP jobs, and browser routes never enter the AtlasMachine or Projection workspace interfaces.

This division preserves the two-entry interface accepted in [Define the TUI navigation, focus, and history state model](03-define-navigation-focus-history-model.md):

```rust
AtlasMachine::start(
    workspace: Arc<ProjectionWorkspace>,
    start: AtlasStart,
) -> Result<(AtlasMachine, AtlasStep), AtlasFault>

AtlasMachine::advance(
    &mut self,
    event: AtlasEvent,
) -> Result<AtlasStep, AtlasFault>
```

`AtlasStep` continues to carry one immutable semantic scene and ordered effects. Enrichment extends its closed protocol rather than creating another caller-facing module:

```rust
struct EnrichmentKey {
    request: RequestId,
    visit: PageVisitId,
    snapshot: SnapshotId,
    base: RevisionKey,
    page: PageEntityId,
    target: DeepInspectionTarget,
}

enum AtlasEffect {
    RunEnrichment {
        key: EnrichmentKey,
        base: RevisionView,
        policy: ResourcePolicy,
        cancel: CancelToken,
    },
    Quit,
}

enum EnrichmentSignal {
    Progress {
        key: EnrichmentKey,
        progress: EnrichmentProgress,
    },
    Finished {
        key: EnrichmentKey,
        result: Result<EnrichmentCompletion, EnrichmentError>,
    },
}
```

The worker executes the already-accepted `ProjectionWorkspace::enrich` operation against the exact immutable `base` handle and emits exactly one terminal `Finished` signal. It may replace intermediate progress in one bounded mailbox slot, but it may never drop or reorder the terminal signal.

### Shared eligibility disposition

Atlas must not reconstruct eligibility by combining allocation, availability, support, and coverage fields itself. The Page projection supplies one presentation-neutral disposition derived by the Projection workspace:

```rust
enum PageEnrichmentDisposition {
    Eligible { target: DeepInspectionTarget },
    Complete,
    Opaque { support: PageDetailSupport },
    Unavailable { availability: Availability },
    SnapshotInvalidated,
}
```

The disposition describes whether work should be offered; `enrich` still rechecks all prerequisites and the exact writable head to close races. Automatic behavior is exhaustive:

| Current Page facts | Atlas behavior |
| --- | --- |
| Available, `semantic` or `structural-only`, and no committed deep result | Start one automatic bounded attempt |
| Product support is `opaque` | Start no work; render the typed support reason |
| Availability is unreadable, unsupported, or encrypted-opaque | Start no work; retain the typed availability |
| Valid, partial, envelope-only, or diagnostic-bearing deep result is committed | Treat it as complete for automatic admission |
| Snapshot is invalidated | Start no work; retained facts are diagnostic evidence only |
| This Page visit already settled through cancellation, refusal, or fault | Start no further work until explicit retry |

This intentionally does not copy the current browser predicate `detail_support.state === 'known'`, because that predicate also matches the known value `opaque` and currently produces an explicit unsupported request. The web adapter's observable request/response behavior remains unchanged by this ticket; Atlas uses the stronger shared typed disposition.

Allocation class does not independently decide eligibility. An Entity that is a valid navigation target remains selectable even when detail is unavailable, while the shared disposition prevents unsupported inspection.

### Visits, admission, and bounded state

Entering a Page creates a private `PageVisitId`. At most one automatic attempt is permitted for `(visit, exact base revision, target)`. Redraw, successful frame presentation, resize, filter changes, scrolling, Page-region changes, overlay changes, and progress signals do not create another attempt. Leaving and later re-entering creates a new visit. An explicit retry creates a new request within the current visit without requiring navigation.

Atlas retains constant-size lifecycle state equivalent to:

```rust
enum AtlasEnrichmentState {
    Quiescent,
    Working {
        key: EnrichmentKey,
        cancel: CancelToken,
        progress: EnrichmentProgress,
        adoptable: bool,
    },
    Draining {
        key: EnrichmentKey,
        cancel: CancelToken,
        progress: EnrichmentProgress,
    },
    Settled {
        visit: PageVisitId,
        base: RevisionKey,
        target: DeepInspectionTarget,
        outcome: EnrichmentNotice,
    },
}
```

There is one physically admitted worker and no FIFO Page queue. When navigation identifies another eligible Page while the prior request is draining, the scene shows that it is waiting for prior cancellation; the current Page is the sole replaceable next intent. Another navigation replaces that intent. After the prior terminal signal releases admission, Atlas re-evaluates only the then-current Page, and starts it only if that visit has not attempted the same exact-base target and its displayed revision is still the writable workspace head.

The Projection workspace is authoritative for admission. An adapter-side idle state never overrides a shared `AdmissionRefused` result, and Atlas never rebases a request from its displayed revision to an implicit latest revision.

### Visible loading and progress

The complete Volume → Sector → Page scene remains projected from `key.base` while enrichment runs. Candidate facts, candidate diagnostics, and candidate coverage do not enter the scene before immutable publication and successful adoption. The title and breadcrumb continue to name the exact displayed revision.

Progress is advisory and presentation-neutral:

```rust
struct EnrichmentProgress {
    phase: EnrichmentPhase,
    evaluated: u64,
    conclusive: u64,
    trusted_total: Option<u64>,
}
```

Atlas may display a percentage only when `trusted_total` exists. Otherwise it displays the phase and trusted counts without inventing a denominator or estimated completion time. Counts are monotonic within one request and must satisfy `conclusive <= evaluated` and, when present, `evaluated <= trusted_total`. Progress can be coalesced to the newest valid value for that exact key; progress with a wrong request, visit, Page, target, snapshot, or base is ignored, and no progress may affect state after the terminal signal.

Escape exposes cancellation as the highest-priority non-modal action already accepted by ticket 03: the first Escape on a working Page cancels and stays on the Page; a later Escape may ascend after the active request has been deactivated. Breadcrumb, selector, sibling, finding, Volume, and quit transitions deactivate before changing the Atlas trail. Resize, filter, scrolling, region changes, and non-navigation overlays do not cancel.

### Cancellation and event ordering

When terminal input and a worker signal are both ready, the event loop delivers one ready user-input event first. Consequently Escape, navigation, or quit wins the Atlas adoption race against a simultaneously ready completion.

Cancellation is idempotent and ordered:

1. Atomically set the shared `CancelToken`.
2. Immediately set `adoptable = false` before any trail transition.
3. Keep the old exact revision displayed.
4. Enter `Draining` until the worker's one terminal signal releases physical admission.

Deactivation controls presentation authority; it does not roll back immutable publication. If Page work observes cancellation before a publication boundary, it returns cancellation without publishing a Page revision. Target-specific linked work such as an OOS value chain may publish the validated prefix and partial coverage allowed by the Projection workspace contract, but only after the same final source/head validation required for every candidate. Cancellation before work is admitted or before any work begins publishes nothing.

If publication commits before the worker observes cancellation, the revision remains in the Projection workspace. A terminal signal received in `Draining`, after navigation, or for any nonmatching key cannot auto-adopt it. Late progress is discarded; a late publication becomes an exact revision offer.

Quit deactivates and signals cancellation before the terminal session is torn down. The worker owner drains or joins the one bounded worker and never leaves background volume access detached from the Projection workspace. Terminal restoration remains the terminal host's responsibility and does not confer adoption authority on a completion received during exit.

### Publication boundary and race precedence

The Projection workspace must apply one publication protocol to valid detail, diagnostic decode failure, resource-limited prefix, interrupted prefix, and terminal invalidation. Existing early diagnostic or partial-return paths must not bypass it.

Before any candidate revision becomes visible, the workspace:

1. Revalidates the snapshot input fingerprint after the candidate work.
2. If source mutation is observed at that boundary, discards the candidate and publishes or reuses the terminal invalidated state for the snapshot.
3. Applies the target-specific cancellation rule; Page cancellation has no candidate, while a linked target may retain only a semantically publishable validated prefix.
4. Confirms that the supplied exact base is still the writable head under the workspace's one-work admission guard.
5. Atomically publishes either the candidate or no revision, increments the revision exactly once when new facts are committed, retains all prior revisions, and returns its exact handle.

Thus source invalidation observed at the final boundary wins over a simultaneously pending cancel, stale base, decode diagnostic, or partial candidate. A preflight cancellation may return before source access and publishes nothing. Once a revision has published, later cancellation cannot remove it. A head mismatch returns typed `StaleBase { current }` and never manufactures a second revision with the same successor number.

This closes current implementation hazards in which Page decode-failure and OOS partial paths can return before the final source check, repeated diagnostic work can append duplicate diagnostics, and independent callers can derive the same successor revision without shared head arbitration. A committed valid, partial, or diagnostic result is idempotent for that target; repeated enrichment returns `Unchanged` rather than duplicating facts or diagnostics.

### Completion and atomic adoption

`EnrichmentCompletion` remains the Projection workspace result accepted by ticket 02:

```rust
enum EnrichmentCompletion {
    Published { revision: RevisionView },
    Unchanged { revision: RevisionView },
}
```

A completion may auto-adopt only when all of these still match the active `Working` request: request id, Page visit, Page and target, snapshot, exact base revision, and `adoptable == true`. Matching is typed; diagnostic messages, subject strings, paths, labels, and revision ordering guesses are never parsed to decide relevance.

Adoption is one transaction:

1. Use only the exact returned `RevisionView`; never check out `latest`.
2. Reproject Volume, Sector, Page, diagnostics, coverage, and every current Atlas-trail ancestor.
3. Verify that every frame has the one expected snapshot and returned revision and that all ancestry and Entity references remain valid.
4. Resolve semantic content anchors against the candidate without mutating current state.
5. Build the complete replacement semantic scene.
6. Atomically replace the one global displayed revision and all projections.
7. Drop projection caches, prepared render frames, and the installed layout generation.

The transaction preserves Volume/Sector/Page identities, prospective focus, committed selection, the normalized filter, finding occurrence, active Page region, modal overlay, and every independently stored semantic content anchor. Only content that vanished in the new projection resolves to its accepted nearest predecessor and clamp rule. Any checkout, projection, identity, ancestry, or scene-building failure retains the entire old scene and exposes the exact returned revision as an offer. Partial adoption and mixed-revision caches are forbidden.

After successful adoption, normal ascent uses the new global revision and restores the accepted Atlas-trail focus and anchors. Atlas does not add revisions to its structural navigation history. Navigation away before completion leaves the old global revision displayed; returning to the original Page is a new Page visit, not chronological revision navigation.

### Revision offers and explicit recovery

A **Revision offer** is one exact immutable revision which exists in the Projection workspace but lacks automatic-adoption authority. Atlas stores at most one offer for the current snapshot, including its exact key and semantic reason. A newer exact offer may replace an older one; a revision from another snapshot is rejected. Atlas never converts an offer into `latest`.

An offer is created when:

- a `Published` completion arrives after cancellation, navigation, quit initiation, or another deactivation;
- an adoption transaction fails after a valid exact completion;
- a typed stale-base response supplies the workspace's exact current head; or
- another explicit workspace response identifies an exact adoptable revision without authorizing a switch.

An explicit semantic `AdoptRevisionOffer` action checks out exactly the offered key and runs the same whole-trail adoption transaction. It operates from the user's current Atlas trail and does not navigate back to the Page that started the work. Failed adoption retains both the old scene and the offer with a typed reason. If adopting a newer revision leaves the current Page eligible at that new base, normal reconciliation may create its one automatic attempt for the new `(visit, base, target)` identity.

Retry is a distinct explicit semantic action. It is available after cancellation, admission refusal, executor submission failure, a pre-publication resource limit, or a recoverable inspection fault. It never silently adopts an offer, silently changes the base revision, or loops on a timer. Unsupported, opaque, unavailable, already-committed, and invalidated dispositions do not expose an enrichment retry until their typed facts change in another explicitly adopted revision.

### Outcome matrix

| Terminal result | Publication | Active matching request | Deactivated or nonmatching request |
| --- | --- | --- | --- |
| New valid Page detail | `Published` | Adopt exact revision atomically | Retain old scene; create exact offer |
| Structural decode diagnostic | `Published` diagnostic revision | Adopt and show diagnostic without stealing Page-region focus | Retain old scene; create exact offer |
| Validated resource/cancel-limited chain prefix | `Published` partial diagnostic revision | Adopt exact partial revision | Retain old scene; create exact offer |
| Target was already committed | `Unchanged` | Complete against exact returned revision; adoption is a no-op when already displayed | Ignore when identical; otherwise offer exact revision |
| Page cancellation before publication | No revision | Settle on old revision; explicit retry allowed | Release admission only |
| Admission refused | No revision | Old revision plus retryable typed notice; no automatic retry | Ignore stale result |
| Resource limit before a publishable Page result | No revision | Old revision plus exact resource notice; explicit retry only | Ignore stale result |
| Unsupported/unavailable race | No revision | Old revision plus typed disposition; no retry until facts change | Ignore stale result |
| Stale exact base | No new revision | Old revision; offer supplied exact current revision | Ignore unless it supplies a newer valid offer |
| Source mutation | Terminal invalidated revision/state | Adopt exact matching result and disable further work | Do not auto-adopt; apply terminal snapshot overlay and offer exact invalidating revision |
| Wrong snapshot, missing revision, or missing Entity | No revision | Preserve old scene and show typed nonsemantic notice | Ignore stale result |
| Inspection/executor fault before publication | No revision | Preserve old scene, release admission, and allow explicit retry when recoverable | Ignore stale result |

Diagnostic-bearing, invalidated, and resource-limited-prefix revisions are successful immutable publications rather than transport or executor failures. Messages and diagnostic subject strings never select a row in this table.

### Snapshot invalidation

Input invalidation is snapshot-wide, not a Page error. Once the Projection workspace observes and publishes terminal invalidation, it admits no more enrichment for that snapshot. Every retained exact revision remains immutable, but projections carry the shared terminal invalidation overlay and present retained facts as diagnostic evidence only.

A matching active invalidation completion is adopted through the normal exact transaction. A late invalidation completion cannot silently replace the displayed exact revision, but the snapshot-level terminal overlay applies immediately, disables work, cancels any replaceable next intent, and exposes the exact invalidating revision as an offer for its detailed diagnostics. This overlay is explicit snapshot state, not a mixture of candidate Page facts with an old revision.

### Web compatibility boundary

The `Live inspection session` remains a web adapter over the shared Projection workspace. This ticket does not import HTTP jobs, URL construction, browser Back/Forward, fetch cancellation, or response codes into Atlas types, and does not redesign current web behavior.

Preserve these observable contracts while extracting the shared publication rules:

- exact snapshot/revision URLs and retained old-revision queries;
- no projection response assembled from more than one revision;
- synchronous internal enrichment represented by the existing `202` completed receipt, `Location`, exact result revision, and exact result URL;
- structured stale-base and invalidation conflicts, resource/admission refusal, unsupported target response, and diagnostic-bearing successful revision;
- revision-bound cursors and cache invalidation only after the web adapter explicitly adopts the returned exact revision;
- terminal invalidation overlay on retained old views; and
- the current explicit opaque-request response, even though Atlas will not initiate such a request automatically.

The web adapter may later consume the stronger shared eligibility disposition only under separately asserted compatibility tests. This decision does not add an HTTP cancel route or change browser history.

### Compatibility gates

Add tests through the Projection workspace, `AtlasMachine::start/advance`, and existing web adapter surfaces rather than exposing private lifecycle helpers:

- Exercise the complete eligibility matrix: semantic, structural-only, opaque, unreadable, unsupported, encrypted-opaque, valid complete, partial complete, diagnostic complete, and invalidated.
- Prove exactly one automatic effect per `(Page visit, exact base, target)` and none from redraw, presentation commit, resize, filter, scroll, region, overlay, or progress transitions.
- Prove one physical admission, a replaceable current-Page intent while draining, no FIFO accumulation, and no overlap between a cancelled worker and its successor.
- Assert that the entire visible scene remains on the exact base revision while trusted progress advances; exercise unknown totals, progress coalescing, monotonic counts, wrong-key progress, and terminal delivery.
- Replay Page cancellation before publication, OOS cancellation with a validated-prefix publication, preflight cancellation with no publication, cancellation after publication, and input-before-simultaneous-completion ordering.
- At the Projection workspace seam, prove a final source/head check for valid, decode-diagnostic, resource-limited, and interrupted candidates; source invalidation precedence; one successor revision; diagnostic idempotence; and no duplicate publication from the same base.
- Exercise matching `Published`, `Unchanged`, decode-diagnostic, validated-prefix, and invalidated completions, plus wrong request, visit, Page, target, snapshot, and base signals.
- Fault every adoption phase and prove rollback to the wholly old scene plus an exact offer; successful adoption must reproject the full trail and preserve all accepted identity, focus, filter, finding, overlay, region, and anchor state.
- Prove ascent after adoption uses the new global revision, while navigation away during work stays on the old revision and cannot be reversed by a late result.
- Exercise exact late and stale-base offers, bounded replacement by a newer same-snapshot offer, explicit adoption from a different current Page, offer failure retention, and the prohibition on any `latest` lookup.
- Prove admission refusal, pre-publication resource limit, unsupported race, executor submission failure, and cancellation never spin or retry automatically; explicit retry starts only one new exact-base request.
- Exercise quit and terminal-fault cancellation so no worker retains detached access and no exit-time completion can adopt.
- Property-test arbitrary event/signal traces for one displayed revision, at most one admitted worker, one terminal signal per request, bounded progress and offer state, valid Atlas ancestry, and no adoption without an exact active match or explicit offer action.
- Retain the web `202`/`Location`/result revision, old URL history, structured stale/invalidation conflict, admission/resource refusal, explicit opaque/unsupported request, diagnostic-revision, revision-bound cursor, and terminal-overlay gates.

No production implementation is made by this resolution. No new ticket is created: semantic progress styling and controls remain within [Define semantic color, glyph, and fallback mappings](06-define-semantic-terminal-rendering.md), numeric queue, memory, and latency ceilings remain within [Set volume viewport and rendering resource budgets](07-set-viewport-resource-budgets.md), and complete cross-adapter verification remains within [Define the parity verification contract](08-define-parity-verification-contract.md). The only new domain term is `Revision offer`, recorded in [`CONTEXT.md`](../../../CONTEXT.md); request ids, Page visits, worker states, and protocol variants are private implementation vocabulary.
