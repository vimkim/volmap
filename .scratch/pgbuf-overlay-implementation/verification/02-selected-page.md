# Ticket 02 verification

2026-09-10. Scope: authenticated selected-page observations from a deterministic
producer through the production UDS decoder, broker, HTTP API and browser.
Task-start source: `c577f344ddc4a3a96d39d77c11c6cffda7367501`.

## Producer prerequisite

The producer-owned v1 contract and revision-2 corpus are copied verbatim from
CUBRID worktree commit `f8c068f771ccd141a3c2f08541fe75c84e41ce53`.
`diff -qr` against that worktree's `docs/pgbuf-inspector/v1` passed.
The offline `vendored_producer_revision_and_every_corpus_byte_match_the_pin`
test verifies all 106 checksummed files and the pinned checksum-list digest
`11dbecc72e4b9dd78e22080f138c801c189c7e047c0db23af8233f44a804d5ce`.
The newer producer delivery supplies the external prerequisite; this does not
claim that its engine changes have merged into CUBRID develop.

## Executed behavioral evidence

- `canonical_exchange_corpus_validates_all_server_stream_outcomes`: all 39
  chronological exchanges, including complete/partial scans, EOF, count and
  sequence violations, duplicates and additive fields. The pinned absent-state
  case additionally asserts that normalized serialization omits unknown fields.
- `wire_frame_limits_include_newline_at_below_and_above_boundaries` and
  `raw_record_slot_and_framed_scan_limits_are_checked_before_deduplication`:
  below/at/above 4 KiB, 64 KiB, 65,536 records and 64 MiB; duplicate JSON keys,
  depth 16 and ignored large-number syntax have separate tests.
- `actual_private_socket_accepts_chunked_producer_and_rejects_unsafe_mode` and
  `peer_credentials_reject_another_effective_uid_without_a_root_exception`:
  actual sockets and Linux credential isolation, with no silent environment skip.
  `broker_refuses_each_identity_component_and_incomplete_volume_sets_before_scanning`
  exercises six full-identity refusal cases.
- `actual_io_deadlines_bound_stalled_handshake_and_scan` checks real 500 ms
  attachment and two-second scan deadlines. The HTTP stalled-body test checks
  that the 2.5-second request deadline starts before body acquisition finishes.
- `broker_shares_capture_charges_held_responses_retains_original_age_and_expires`
  verifies eight retained responses, rejection of a ninth, unchanged original
  age after an unfinished scan, rejection of reused sequence after reconnect,
  and broker expiry at 30 seconds.
  `eight_inflight_callers_share_one_slow_capture_and_reject_a_ninth` verifies
  concurrent waiter coalescing across a capture older than the 500 ms floor.
- `aggregate_boundary_counts_retained_inflight_and_response_owners_until_last_drop`
  checks below/at/above the 128 MiB reservation boundary and surviving-reference
  lifetime. Fixed scalar records bound a decoded slab below 16 MiB; its 32 MiB
  reservation is below the 48 MiB object ceiling. These are conservative
  allocation bounds, not measurements of process RSS.
- `observation_http_bounds_ordered_scopes_and_request_bytes`: ordered scopes
  0/1/511/512/513 and request sizes 65,535/65,536/65,537 bytes.
  `selected_page_observation_crosses_real_socket_and_http_with_bound_identity`
  covers authenticated normalized HTTP evidence, no-store and intact disk state.
  `held_http_response_bodies_keep_admission_until_released` exercises the actual
  response-body owner through the HTTP handler and verifies overload status 429.
- Seven observation frontend tests cover explicit demand, route/epoch/scope and
  generation rejection, pause, full-round-trip age, 29,999/30,000 ms expiry,
  backwards clocks, 999/1,000/1,001 ms freshness and unknown versus nullable state.
  The selected-page browser case uses an actual UDS producer and HTTP server in
  both Chromium and Firefox; it checks residency, non-color labels, detail,
  pause, disable, unchanged Page facts, no-store and absence of page errors.

## Review

The code-review skill ran independent Standards and Spec reviewers against the
proposed task-start baseline. Both reviewed immutable candidate snapshots;
`38db23cc80d811075b6474861170ce2c1fc39f2a` contains the reviewed fixes.

Standards: zero unresolved substantive findings. The unknown/known-empty
contract violation is fixed. A typed error enum remains optional cleanup.

Spec: zero unresolved substantive findings. Missing state serialization,
HTTP overload classification and the 500 ms selected-page freshness interval
are fixed with regression coverage. A subsequent corpus-test helper extraction
only satisfies Clippy's function-length limit.

## Remaining scope

Automated lifecycle scheduling, viewport demand, heatmap topology, full density
and accessibility gates, live CUBRID integration, and release performance
readiness remain tickets 03–08. This slice supplies explicit selected-page refresh.

## Final gate

`just verify` exited 0. Main Rust suite: **293 passed, 3 existing ignored**;
frontend: **55 tests in 8 files passed**; Playwright: **7 passed, 1 existing
Firefox skip**, including the new producer journey in both browsers.
Formatting, all-target/all-feature Clippy, static-musl ELF checks, frontend
types, generated-artifact reproducibility, advisories, notices/SBOM checks,
locked metadata and whitespace checks passed. The two supply-chain-specific
Rust test invocations also passed (one case each).

The full command log is [02-just-verify.log](02-just-verify.log).
Screenshots were reviewed after moving observation detail beside Page facts:
[Chromium](selected-observation-chromium.png) and
[Firefox](selected-observation-firefox.png).
