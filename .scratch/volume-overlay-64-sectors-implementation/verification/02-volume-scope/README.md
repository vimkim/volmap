# Ticket 02: 64-sector Volume observation handoff

Source baseline: `6f90d05c696ed21e8b93398e9aa957d76dd00f93` (2026-09-11).
The pre-existing edit to producer ticket 07 is outside this change.

## Contract

Volume sends `scope: {kind: "volume", volid, sectorids}` with at most 64 unique
sector IDs, plus epoch, generation, cadence and demand/retry flags. It sends no
explicit VPID list. The UI selects intersecting visible sector cards by Euclidean
centre distance to the viewport centre, breaks ties by sector ID, and sorts the
selected IDs before requesting. Changes to distance order within that same set
preserve the current batch; scroll/resize can change the set. No periodic rotation
or nonvisible fill is used. An empty physical-page selection sends no request.

The response uses `volmap.runtime.page-buffer.scoped`, schema version 1,
variant `volume-residency-lru`, and echoes the canonical scope. Its flat `slots`
array has exactly `64 * sectorids.length` entries. For slot `i`, the page address
is `sectorids[floor(i / 64)] * 64 + i % 64`. A null slot is absent physical storage,
not unknown residency. Rows contain `state`, `reason` and nullable `evidence`;
resident evidence contains only optional `lru_zone`, `lru_list_kind` and
`lru_list_index`. An omitted index is unknown, null is no membership/not applicable,
and zero is a valid index. Empty resident evidence preserves resident/LRU-unknown.

One response looks up one shared capture and carries capture metadata once.
The client validates the scope, epoch, generation, slot count, classifications,
coverage and topology before adopting the entire batch. Producer completeness is
separate from requested/evaluated coverage. Capture identity does not imply an
atomic scan. Pause, expiry, incarnation refusal and late-response rejection use
the existing lifecycle. Sector detail, selected Page detail, legacy 512-VPID
requests, collection pagination and producer wire v1 retain their contracts.

Volume tooltips, legend and disclosure table omit dirty/flushing and expose
LRU kind/index and unknown values explicitly. Sector/Page retain detailed evidence.

## Resource evidence

Actual public HTTP serialization over the authenticated fixture socket:

| Case | Request bytes | Response bytes |
| --- | ---: | ---: |
| 4,096 resident rows; maximum LRU counts/index, epoch, capture sequence/timestamps | 299 | 554,599 |
| Same scope with optional/unknown, null and zero LRU evidence | 299 | 554,510 |

See [HTTP raw output](http-size.log). These are serializer results, not size models.
The 64 KiB request and 1 MiB response caps remain unchanged. Request body tests
exercise 65,535/65,536/65,537 bytes for Volume as well as Sector and legacy requests.
The bounded server writer retains its 1 MiB capacity and rejects an extra byte;
the HTTP client now also stops consuming response bodies beyond 1 MiB, including
responses without Content-Length. Oversized responses cause whole-batch rejection.

`volume_projection_capacities_fit_the_existing_request_reservation` conservatively
sums even mutually exclusive lifetimes: 4,096 original rows, fixed slots, compact
slots, PageKeys and VPIDs; 64 sector IDs; the 1 MiB output capacity; 64 KiB body
allowance; and 16 KiB metadata allowance. See [the allocation calculation output](memory.log). Vector storage is 753,664 bytes; total
is 1,884,416 bytes, below the unchanged 2,097,152-byte per-request reservation.
The response owns its reservation and admission permit through body consumption.
Existing aggregate and real-decoder tests retain the 128 MiB accounting boundary
with retained capture, replacement assembly and eight response owners. Neither
producer caps nor broker admission/capture caps were increased.

[Concurrent HTTP evidence](concurrency.log) mixes Page/legacy, Sector and 4,096-page
Volume scopes against one gated capture, verifies complete/partial/duplicate
classification, explicit unevaluated legacy pages, ninth-request refusal and
independent disk access.

## Address and lifecycle boundaries

Public HTTP tests cover 0/1/63/64/65 sectors, reordered and noncontiguous selections,
duplicates, negative/nonexistent identifiers and overflow. Decoder tests cover
maximum valid VPID addressing, 4,096/4,095 non-null slots, a null hole without
address shifting, and invalid 4,097-slot/65-sector structures. Canonical set reuse
and no-request empty selection are exercised at the existing reducer seam.

As documented in ticket 01, the current volume format validator rejects a
4,095-page physical file with `volume.header.file_length` before an inspection
projection exists. This change preserves that input contract. The 4,095-slot
case is decoder coverage; it is **not** evidence that HTTP now accepts short
physical volumes. No page is invented to fill such an input.

Browser regression includes mixed detail/Volume tabs, complete/partial/duplicate/
unevaluated states, stale/expiry, pause/late response/resume, incarnation change,
source failure, overload, malformed scope/slots/epoch/generation/evidence and
oversized-body rejection. Existing broker virtual-clock tests continue to cover
shared cadence, scan-start demand, cancellation and clock discontinuities.

## Browser evidence and verification

The new `web/e2e/volume-observations.spec.ts` uses port 41743 with the existing
sparse dense disk fixture (192 complete sectors) and authenticated socket fixture.
The producer emits 12,288 resident records with changing LRU zones each capture;
the public HTTP query projects 4,096 of them. No API route interception supplies
the successful dense-flow data. Scope changes may cancel the last scan observer;
the fixture survives the resulting connection close.

Separate Chromium and Firefox JSON records report actual rendered cells, visible
sectors, queried pages, visible resident cells, wire bytes, selected/reselected
sector IDs and capture identities. The test checks focus preservation across
refresh, state/LRU switching, the 4,096-row disclosure table, and scroll/resize.
Screenshots show the LRU projection and disclosure. These functional checks are
not formal performance qualification; 1/8/32-tab timing, CPU/RSS and real-producer
workloads remain the follow-up tickets' responsibility.

The initial browser regression run passed 52 tests and failed two old Volume
assertions expecting `2DF` (dirty/flushing) rather than the new lightweight `2`.
Those assertions were updated while preserving the Sector expectation. Raw output
is retained in [browser-initial.log](browser-initial.log).

Final `just verify` passed: Rust 319 tests, Vitest 78 tests, browser 83 passed
and one existing skip; Rust formatting, Clippy, static musl ELF verification,
frontend toolchain/advisory/artifact checks, supply-chain metadata and diff checks
also passed. See [the final raw log](verify.log) and [two-axis review](review.md).
Standards: 0 actionable findings. Spec: 0 actionable findings.

The first full run also found an exact-title mismatch in the new Volume lifecycle
assertion, a Firefox second-tab navigation wait, and an attempt to inspect a
response canceled during scope selection. Those test conditions were corrected;
[verify-initial.log](verify-initial.log) retains the non-passing run. Successful
adoption assertions now wait for completed, current-scope responses and verify
the second capture's scan sequence and all 4,096 rendered LRU zones.

The existing optional density diagnostic's old fixed-512 readiness assertion was
updated to positive equal evaluated/requested counts, because viewport selection
now determines the scope. That diagnostic was typechecked but **not rerun or
performance-qualified** here. The follow-up ticket owns its formal measurements.

| Browser | Actual rendered cells | Visible sectors | Queried pages | Visible resident cells | Request/response bytes |
| --- | ---: | ---: | ---: | ---: | --- |

| [chromium](browser-chromium.json) | 12,288 | 120 | 4,096 | 4,096 | 322 / 515,604 |
| [firefox](browser-firefox.json) | 12,288 | 120 | 4,096 | 4,096 | 322 / 515,604 |

Screenshots: [Chromium](volume-chromium.png), [Firefox](volume-firefox.png).
[Execution provenance](provenance.json) records the functional fixture/build context.

Stored text logs normalize trailing whitespace and final blank lines for Git;
commands, outcomes, numbers and failure diagnostics are unchanged.
