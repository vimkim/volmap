# Ticket 04 — Shared visible-page demand verification

Date: 2026-09-10. Starting commit: `9befbd2e1736c9584f8bfca2c88f1ad9f310ea30`.
The consumer revision is the commit containing this record. The producer
protocol and pinned conformance corpus are unchanged.

## Delivered behavior

Volume observations follow sectors intersecting the viewport, ordered by
proximity to its vertical centre and then physical sector/page order. Sector
observations request its physical pages. A selected Page takes the first slot
when building a scope; remaining pages rotate explicitly if the scope exceeds
512 VPIDs. Volume/Sector poll at two seconds and selected Page at 500 ms.
Viewport changes revoke the request epoch and old batch before requesting the
new scope. Empty viewports create no observation HTTP demand.

The shared broker from tickets 02–03 continues to own one connection, one
latest capture and one in-flight refresh. Every admitted waiter keeps its own
ordered scope, epoch, cadence and evaluated count. A completed in-flight scan
may serve its waiters; later cache users apply their own cadence. Browser
coverage is independent of producer completeness, and partial scans are never
combined to infer absence. No starvation-free guarantee is made for arbitrary
numbers of clients.

Volume and Sector expose admitted/visible and evaluated/requested counts,
producer completeness, conservative age/freshness, available page observations
and capture limitations. Sector page cells include text residency labels;
Volume's expandable table contains per-page residency, dirty and flushing
state. Full heatmap/LRU visual encoding remains ticket 05. HTTP 429 has an
explicit observation-overload message and bounded retry; disk navigation keeps
its independent admission.

## Verification

Final `just verify`: **passed (exit 0)**. **306 Rust tests, 73 frontend
unit tests and 17 browser cases passed**, with three pre-existing Rust ignores
and one pre-existing Firefox skip. All 12 producer-backed browser cases run
in both Chromium and Firefox. See [04-verify.log](04-verify.log).

| Contract | Executed evidence |
| --- | --- |
| Selected-first scope and overflow | `viewport demand preserves selected priority, rotates overflow, and revokes late batches`: selected VPID stays first, 600-page stable viewport progresses across two bounded requests, old epoch cannot adopt |
| Default cadence and cap edges | `visible-page cadence is two seconds with explicit below, at and above-cap coverage`: 511/512/513 pages; existing selected-Page polling and per-caller cache/floor tests |
| Complete batch validation | `every batch row and scope coverage is validated independently of producer completeness`: malformed later rows, mismatched counts and false absence are rejected |
| Eight simultaneous callers | `eight_inflight_callers_share_one_slow_capture_and_reject_a_ninth`: different scopes/epochs, 500/2000 ms cadences, a shared slow scan, independently unevaluated pages |
| Real HTTP and independent disk work | `concurrent_http_scopes_share_scan_but_keep_coverage_and_disk_admission_independent`: eight HTTP waiters, one truncated producer scan, per-waiter ordered scope and epoch, ninth HTTP 429, successful session/volume/capability requests while scan is blocked |
| Cancellation and no-demand shutdown | Existing `cancellation_preserves_other_callers_and_last_observer_closes_unfinished_scan`: cancelling one caller preserves another; the last cancellation closes unfinished producer work |
| Request/response ceilings | Existing `observation_http_bounds_ordered_scopes_and_request_bytes`: 0/1/511/512/513 VPIDs and 65,535/65,536/65,537 body bytes. `normalized_response_cap_includes_all_serialized_bytes_without_growing_past_it`: 1 MiB minus one / exact / plus one bytes |
| Response-held admission | Existing `held_http_response_bodies_keep_admission_until_released` and broker held-response tests keep request permits and allocation charges alive with serialized bodies |
| Shared memory admission | `actual_decoders_cannot_admit_object_caps_that_exceed_the_shared_total`: real decoders, retained/old/in-flight captures and eight serialization reservations; refusal before allocation; final byte below/at/above aggregate cap |
| Truncated rotation and absence | `rotating_truncated_captures_never_accumulate_absence_or_hide_duplicate_ambiguity`: a stable six-slot pool advances in two-slot captures; earlier residency never becomes absence evidence; a later complete scan preserves duplicate ambiguity |
| Producer cap boundaries | Existing `raw_record_slot_and_framed_scan_limits_are_checked_before_deduplication`: 65,535/65,536/65,537 records and visited slots; framed 64 MiB minus one / exact / plus one; exact-cap complete footer is accepted |
| Real browser behavior | Chromium and Firefox cover Volume/Sector coverage, available states, concurrent selected-page tab, overflow rotation, viewport churn, overload and working disk navigation. All existing lifecycle/late-adoption scenarios also run |

The gate checks Rust formatting/tests/Clippy, static-musl build and ELF,
frontend types/tests, reproducible generated JS/CSS and supply-chain files,
advisories, Chromium/Firefox, Cargo-only embedding, metadata and whitespace.
Producer-backed browser navigation waits for document commit and then checks
rendered controls/evidence. This avoids Firefox document-readiness waits that
can remain pending alongside the live watch without weakening UI assertions.

The existing ignores/skips are manual TUI/resource tests and the
Chromium-owned legacy parity corpus; none replaces a ticket-04 scenario.

### Memory accounting argument

The shared session reserves 8 MiB, each live decoder reserves 16 MiB of parser
scratch, and each capture reserves 32 MiB before allocating its bounded record
slab. A decoded scan plus its scratch is therefore charged at 48 MiB. Records
contain scalars and static vocabulary, with a compile-time size ceiling of
256 bytes; 65,536 records use at most 16 MiB before sorting overhead. Parsing
is incremental with a bounded frame buffer; no whole-wire buffer or spill is
introduced.

Eight response owners reserve 2 MiB each, including serialization workspace
and the at-most-1-MiB output. Normal peak reservations are 8 + 16 + 32 + 32 + 16
= 104 MiB (session, parser, latest, in-flight, responses). Responses own
serialized bytes rather than retaining captures; serialization borrows the
current capture while holding the session lock. Old-capture ownership is
nevertheless exercised explicitly in the allocation test: with 104 MiB
already charged to session, parser, two captures and responses, another
32-MiB capture is refused. Releasing the old owner permits replacement; the
128-MiB atomic aggregate ceiling continues to apply to every reservation.
These are conservative accounted allocations, not an RSS measurement.

## Review

Independent Standards review: **0 unresolved findings**. Two maintainability
suggestions were addressed: shared cadence-aware metadata presentation and a
validated TypeScript state/reason union.

Independent Spec review: **0 findings**, including a recheck of those changes.

## Visual evidence and delivery boundary

- [Chromium Volume](visible-volume-chromium.png)
- [Firefox Volume](visible-volume-firefox.png)
- [Chromium Sector](visible-sector-chromium.png)
- [Firefox Sector](visible-sector-firefox.png)

These tests use the real Volmap HTTP service and scripted authenticated
AF_UNIX producers. They consume, but do not implement, the producer contract
of at most one visit per slot and rotation beyond the visited span. Real
CUBRID traversal/integration evidence, dense visual/accessibility gates and
release performance acceptance remain tickets 05–08. No page-image
correspondence, commit visibility, durability or event causality is implied.
