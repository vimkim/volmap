# Volmap state-only overlay implementation entry

Status: reviewed handoff input, paired with the CBRD-27398 producer draft and
delivery plan. This is input to `/to-spec`, not
authorization to implement or a claim of passed release gates.

## Scope and authority

Implement the optional web-only CUBRID page-buffer observation source. Keep
ordinary disk inspection, its facts/outcomes/diagnostics, terminal output and
exports unchanged. The [handoff index](README.md) links every accepted decision.
The [architecture](../issues/10-define-volmap-overlay-architecture.md),
[budgets](../issues/15-set-overlay-resource-budgets.md) and
[verification strategy](../issues/11-define-verification-strategy.md) are
normative and take precedence over the broader
[runtime specification](../../volmap-live-web-runtime/implementation-spec.md)
for this source.

Do not make the broader specification's resident-page inspection delivery a
prerequisite for this overlay. No page capture, digest, DWB/TDE comparison,
durability classification, AOUT observation or transition event history is
included. Linux page-cache observation retains its independent contract.

## Implementation ownership and seams

Paths marked planned below are implementation landing points, not existing
modules or tests. Their private organization can change without changing the
accepted interfaces or crossing the source boundaries.

| Owner | Landing points | Responsibility |
| --- | --- | --- |
| Volmap runtime implementer | Planned `src/runtime_observation/`; existing `src/web.rs` | One broker per attached serve session, separate `WebState` ownership, UDS/simulated source adapters, injected clock/scheduler, bounded scan cache and capability mapping |
| Volmap HTTP implementer | `src/cli.rs`, `src/web.rs` | Explicit enablement/socket configuration, loopback enforcement, independent runtime admission, normalized no-store resources |
| Browser implementer | `web/src/model.ts`, `web/src/runtime.ts`, `web/src/api.ts` and relevant view components | State/effect integration, scope authority, selected/visible scheduling, pause/hidden behavior, state marks and LRU display |
| Volmap test implementer | Planned `tests/runtime_observation.rs`, `tests/runtime_http.rs`, `fixtures/pgbuf-inspector-v1/` | Real-parser scripted UDS cases, HTTP/security/cap tests and vendored corpus verification |
| Browser test implementer | Existing model/runtime tests; planned `web/e2e/page-buffer-overlay.spec.ts` | Semantic, lifecycle, density, timing and accessibility evidence |
| Documentation owner | `README.md`, `CONTEXT.md`, ADR-0006 | Explicit attachment instructions, source limitations, stable language and existing web-only boundary |

The private broker exposes capability metadata and bounded observations for
a validated scope. It owns framing, identity, incarnation, retry, scan
assembly and allocation accounting. Neither HTTP handlers nor React should
parse producer frames. Runtime notifications never advance the disk-follow
generation, inspection revision or cursor, or wake disk-watch clients.

## HTTP and attachment contract

- Retain `GET /api/v1/runtime/capabilities` and
  `POST /api/v1/runtime/page-buffer/observe`, both `Cache-Control: no-store`.
  Use a separate runtime envelope, not inspection outcomes or diagnostics.
- Requests carry bounded ordered VPIDs and scope/epoch authority. Responses
  echo accepted scope and report normalized state, incarnation-bound capture
  identity, scan interval, limitations, producer completeness and separate
  requested/evaluated counts. Browser coverage is not producer completeness.
- Attachment requires explicit runtime enablement and an explicit socket
  path; no discovery or environment fallback. Requested attachment with a
  non-loopback HTTP bind is a startup error. Unattached serving is unchanged.
- Follow the full [security decision](../issues/08-set-security-posture.md):
  exact effective-UID agreement with the server, socket and private directory;
  no root/group exception; complete persistent-volume identity verification;
  unpredictable incarnation. Do not substitute path/name matching.
- HTTP/UI disclose only sanitized capability, reason, protocol, fingerprint,
  abbreviated incarnation and timing information. Never disclose socket or
  volume paths, process identities or raw OS errors.

## Capture and lifecycle contract

Use one producer connection and coalesce demand into at most one in-flight
scan. There are no per-tab producer connections or missed-tick queues.
Publish only scans with valid footer/count/framing and accepted limits.
Valid truncated scans are partial; missing pages are unknown. Only omission
from a complete scan supports observed nonresidency, not guaranteed current
absence. Duplicate VPIDs are ambiguous. Never fill a new scan's gaps from
older captures or reconstruct events from differences.

Transient failure discards unfinished assembly and preserves previous usable
evidence with original age until expiry. Identity/peer refusal or protocol
incompatibility clears evidence and requires explicit retry. An incarnation
change invalidates retained evidence and in-flight adoption even while paused.
Disk-generation changes alone do not imply runtime identity changes or prove
page-image correspondence.

Route, scope, overlay and pause changes revoke incompatible browser authority;
cancellation saves work but does not replace response-epoch checks. One tab
cancelling cannot cancel another's demand. With no remaining observer, stop
unnecessary refresh work. Resume requires a scan started after resume.

## Resource and scheduling summary

The [budget decision](../issues/15-set-overlay-resource-budgets.md) owns exact
boundary, accounting and deadline semantics; the following is a navigation
summary, not a replacement for its validation rules.

| Concern | Accepted limit/default |
| --- | --- |
| Producer scan validation | 65,536 visited slots/records; 64 MiB framed scan; 4 KiB record/control frames; 64 KiB handshake; JSON depth 16 |
| Connection/exchange | Connect plus handshake 500 ms; whole scan exchange 2 s; producer traversal/serialization 100 ms checked between slots, not a real-time guarantee |
| Broker memory | 128 MiB total accounted allocations; at most 48 MiB per decoded scan; retained/in-flight/response allocations all count; no spill/history |
| Observation HTTP | 512 VPIDs, 64 KiB body, 1 MiB response; eight admitted requests including waiters, no extra queue; 2.5 s deadline; overload 429 |
| Capability HTTP | Four independent slots, no queue, 1 s deadline |
| Polling | Selected 500 ms, visible 2 s; broker scan-start floor 500 ms; cache reuse within caller cadence |
| Retry | Nominal 0.5/1/2/4/8 s, capped at 8 s, +/-20% jitter (9.6 s actual maximum); reset after valid scan |
| Hidden/paused | Hidden: no requests. Paused: metadata only every 5 s, no scan demand or new evidence adoption |
| Freshness/expiry | Conservative monotonic age; fresh only within two caller intervals; expire after 30 s, including paused evidence |

Do not buffer the complete wire stream or use response timestamps to renew
capture age. Preserve the broker's request-start-based upper age bound and
add browser transport/elapsed uncertainty as specified. Untrustworthy elapsed
time revokes freshness; source wall-clock steps never renew it.

## Visual and vocabulary contract

Preserve storage colors under cyan residency outlines, amber dirty corners
and a static magenta flushing edge. LRU topology is a separate color mode,
with exact kind-local index reachable in selected-Page detail. Latch/waiter/
fix facts belong in detail, not dense-grid animation. Keep fresh, stale,
paused, expired and absent/refused capability presentations explicit and
accessible through non-color carriers.

Retain the accepted glossary and ADR-0006. Do not introduce “live database,”
atomic snapshot, eviction history or durability language for this source.
The documentation owner must remove contradictory older integration wording
as the focused specification is adopted, without changing other sources.

## Required verification and handoff evidence

Use the producer-owned revision/hash-pinned corpus offline. Test framing and
chunking through the real UDS parser, broker scheduling with controlled time,
and HTTP behavior through actual handlers. Cover partial versus malformed
captures, cap boundaries, concurrent demand/cancellation, overload, refusal,
identity changes, expiry, hidden/pause/resume and unaffected disk inspection.

Run Chromium and Firefox semantic tests at Volume and Sector scale; the
10,000-page case must record actual rendered density. Require p95
input-to-visible update at most 100 ms on the recorded reference host,
accepted memory gates and manual screen-reader/visual review evidence.
Screenshots support, but do not replace, semantic/accessibility assertions.

Real-engine checks cover debug and release on the agreed format-aligned
branch; the producer also has develop checks. The full dedicated-host release
performance matrix and exact-commit evidence manifest remain mandatory under
the verification decision. Empty, missing, skipped or inconclusive test runs
are not passing. No such results are claimed by this document.

Next flow after completed cross-repo handoff: `/to-spec` to consolidate these
requirements into the focused build plan, then `/to-tickets` and `/implement`.
Do not bypass that consolidation by treating this index of accepted decisions
as an already-executed implementation ticket.
