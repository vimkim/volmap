# Ticket 01: Sector observation scope

Implementation baseline: `5dacafbb248600fd6a27b6aae8b5a1655cfa12c9` plus the existing working tree on 2026-09-11. Existing listener, tools, design and producer-evidence changes were preserved in commits `aad339f`, `e2b4e20`, `9ad6d01`, and `eb909cf`; the implementation review uses `eb909cf` as its exact fixed point. No producer protocol change.

## HTTP contract

POST `/api/v1/runtime/page-buffer/observe` accepts exactly one addressing form: existing `pages`, or `scope`. Sector requests omit `pages`:

```json
{"scope":{"kind":"sector","volid":0,"sectorid":1},"epoch":"9","generation":"0","cadence_ms":2000,"after_request":false,"retry":false}
```

The new response has `schema: "volmap.runtime.page-buffer.scoped"`, `schema_version: 1`, `variant: "sector-detail"`, the echoed `scope`, and exactly 64 `slots` in physical page-slot order. Slot 0 in sector 1 is page 64; slot 63 is page 127. Every present slot retains the existing detailed row (`volid`, `pageid`, state/reason, nullable evidence). The new response omits legacy `pages` and `observations` arrays. A null slot denotes no physical page and must never shift later addresses. The decoder checks each present row against its slot-derived VPID and counts only present rows.

Epoch, generation, capability, capture identity/incarnation/times/age/topology, requested/evaluated counts, producer completeness and limitations remain once in the envelope. Producer completeness is independent of requested/evaluated coverage. Complete omission is not-resident; partial omission and duplicate VPID are unknown. Detailed dirty/flushing/fix/latch/LRU/LSA evidence remains unchanged.

The server validates sector membership and checked addresses through the requested current generation's inspection view before invoking the existing broker. Negative, unknown, overflowing or malformed addresses fail with HTTP 400; generation mismatch is HTTP 409. Supplying both addressing forms is rejected. Legacy explicit VPID requests retain their ordering, duplicate handling and 512-page cap, including the selected-Page path.

### Short-sector boundary

The implementation experiment found an existing format prerequisite: `format::volume::validate_file_length` rejects a file shorter than `total_sectors * 64 * 16384` bytes with `volume.header.file_length`. Consequently an accepted inspection generation cannot currently contain a short last sector. Ticket 01 preserves this format contract rather than inventing absent pages or widening disk-format support. A 4,095-page file declaring 64 sectors is tested through inspection opening and rejected before HTTP serving. Separately, the scoped decoder test uses null physical slots and proves that slots 0 and 62 of sector 1 remain pages 64 and 126, with requested/evaluated 2. Supporting accepted short volumes is not claimed.

## Lifetime and resource bounds

Sector demand uses the matching Sector inspection projection; navigation cannot reuse the previous Volume viewport as a Sector request. Adoption validates scope, generation, epoch and every ordered VPID. Sector cadence remains 2 seconds; selected Page remains 500 ms. The existing broker owns capture sharing, demand barriers, freshness, expiry, incarnation refusal and retry. Browser tests exercise delayed Sector responses during pause, post-resume demand, expiry and a real fixture-producer restart while paused.

The HTTP 64 KiB request and 1 MiB serialized-response caps, 8 observation admissions, separate disk admission, response-body ownership charge, capture caps and producer v1 protocol are unchanged. Sector addresses expand to at most 64 VPIDs. The new response moves the existing rows into `Option<Row>` slots without cloning evidence or capture metadata; serialization still uses the bounded 1 MiB writer under the existing 2 MiB request/response reservation. The extra scope tag and bounded slot/vector bookkeeping are smaller than the existing 512-row legacy case. This is a bounded implementation argument, not a new RSS or multi-tab performance measurement.

## Executable evidence

- `cargo +1.97.1 test --locked sector_observation_http --lib`: valid and invalid scoped HTTP requests, exact slot addresses, scope/schema/variant echo.
- `cargo +1.97.1 test --locked observation_http_bounds --lib`: legacy 0/1/511/512/513 pages, mutually exclusive addressing, both forms at 65,535/65,536/65,537 body bytes.
- `cargo +1.97.1 test --locked concurrent_http_scopes --lib`: real authenticated socket fixture → mixed Sector/explicit HTTP requests, one gated capture, complete/partial/duplicate cases, separate coverage, ninth-request overload and usable disk requests.
- `cargo +1.97.1 test --locked short_sector_input --lib`: short source rejection before runtime scope construction.
- `mise x node@24.19.0 -- corepack pnpm --dir web exec vitest run src/observations.test.ts`: decoder slots and existing reducer/lifecycle contracts.
- `mise x node@24.19.0 -- corepack pnpm --dir web exec playwright test observations.spec.ts overlay-accessibility.spec.ts`: both browser engines; actual Sector request and detailed display, Page drilldown, malformed schema/variant/scope/slots/epoch/generation, partial/duplicate/complete rendering, focus, pause/resume/expiry/restart, delayed adoption.
- `just verify`: full repository verification, including generated frontend reproducibility and static musl release checks.

Existing wire fixture: `fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl`. The HTTP test binds its hello identity to the actual inspected fixture volume, then varies the footer completeness and duplicate record. Browser tests use the existing `release/run-browser-server.sh` producer fixture, not a real CUBRID server.

## Results (2026-09-11)

Recorded logs retain command output with trailing whitespace normalized for repository checks.

`just verify` passed (see [raw log](verify.log)): 310 Rust unit/integration/doc tests, all-target Clippy, static-musl ELF checks, frontend typecheck, 74 Vitest tests, deterministic generated-artifact comparison, dependency advisories, Chromium and Firefox browser checks, Cargo-only embedding, and whitespace checks. Browser result: 65 passed and one existing skipped case. The initial full frontend run's two obsolete legacy-shape accessibility fixtures failed and were updated to handle Sector slots; [initial log](frontend-initial.log) retains that failure evidence. The final run passes both cases.

The Standards review found no documented violations and one minor misleading error name; `InvalidSector` was renamed `InvalidAddressing`, followed by a focused HTTP test, Clippy, and a rebuilt static-musl ELF check ([post-review log](review-fix.log)). The Spec review found no implementation defects or scope creep, and identified the disclosed short-volume verification limitation above. [Review record](review.md) keeps the axes separate.

Supporting browser captures: [Chromium Sector](sector-chromium.png), [Firefox Sector](sector-firefox.png). Existing historical screenshots were restored after testing. These captures supplement semantic assertions and do not qualify visual performance.

Ticket 01 does not qualify the 4,096-page Volume projection or the later performance gates. Ticket 02 can build on the scoped envelope/discriminator, unchanged legacy path and shared broker demonstrated here.
