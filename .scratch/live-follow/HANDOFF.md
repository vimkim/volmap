# Live follow — handoff

Read `SPEC.md` first, then `issues/01`–`issues/06`. This file records where the
work stopped and what is expensive to rediscover.

Worktree `/home/vimkim/temp/volmap-live-follow`, branch `feat/live-follow`.
Tracked as work-tracker item **5** (`work-tracker show 5`, `work-tracker history 5`).

## State

| Issue | State |
| --- | --- |
| 01 fingerprint, `SourceMode`, `Torn`/`Superseded` | done |
| 02 follow watcher, generation store | done |
| 03 live entity URLs, watch endpoint | done |
| 04 browser follows generations | done |
| 05 `serve --follow` flags | done |
| 06 e2e proof and docs | done |

Commits on top of `main`:

- `6333509` foundations (01, 02)
- `4d6528c` live entity URLs and serve flags (03, 05)
- `e92dfb4` e2e proof over a real socket
- `693dfc4` browser onto live entity URLs (04, first half)
- `65227b9` input disk time, `CONTEXT.md` vocabulary
- `5aee0cf` name browser cancellation counters as epochs
- `0ac24a6` browser follow loop, chip, pause/resume, base-view re-render
- `057066f` slot/OOS enrichment re-issue and mosaic restart
- `0b60125` README live-follow documentation
- `4a63b9e` pure mid-scan classification and both-mode test
- `f655aa3` keep an evicted mosaic frozen while the browser is paused
- `f77c533` prove generation 3→40 URL survival and stale-manifest rescan scheduling

The branch is rebased onto `main` at `a7505b1`, clean, and `just verify`
passes in full: 56 library tests plus every integration suite, clippy with
warnings denied, static-musl ELF checks, metadata/notices, and diff-check.

## Next action

None. On 2026-08-21, the user accepted the committed text-contract and
server-e2e coverage together with the recorded one-off real-browser smoke. No
Chrome, Playwright, or Puppeteer target will be added, and `just verify` remains
the pinned project gate.

## Acceptance record

**Acceptance 4 is covered.** `classify_mid_scan_source_change` is a pure
decision: the unit test asserts `Invalidated` + `snapshot.modified` + fatal for
immutable mode and `Torn` + `snapshot.torn_read` + warning for live mode. No
writer/scan timing race is involved. A second deterministic test starts the
follower with a reading whose recorded manifest is already stale — the state a
mid-scan change leaves behind — and proves that it publishes another scan.

**Acceptance 2 is exact.** The server test resolves `/api/v1/page/0/2` at
generation 3, publishes through generation 40, proves generation 3 has left the
retention window, and resolves the unchanged URL at generation 40.

**The browser automation policy is accepted.** No committed real-browser target
guards the follow behaviour. The automated gate covers the JavaScript as
*text* (asset contracts) and the server over HTTP, while the one-off Chrome
evidence below covers the real DOM. This remaining automation gap is accepted;
browser tooling stays outside the repository and `just verify`.

A one-off Google Chrome DevTools smoke did exercise the real DOM against
reflinked copies of both `demodb` data volumes. It proved: generation 0→1 kept
`/volume/0`, scroll 600, and history length 1; Pause held generation 1 while
reporting newer generation 2 and Resume adopted 2; `/slot/0/130/1` survived
2→3; `/oos/1/4225/0` survived 3→4; and an evicted progressive cursor with
retention 2 stayed frozen while paused, then resumed at the newest generation.
That last case found the pre-`f655aa3` bug where the mosaic silently restarted
under an old paused chip. The rerun after the fix kept 24 old sectors, showed
`Resume to refresh the mosaic`, and only loaded generation 2 after Resume,
without changing history or scroll.

## Expensive to rediscover

**The viewer shows disk state, not committed state.** `CONTEXT.md` now names
this *observed disk state*. A change committed to the log but not flushed to a
data volume is invisible; a page written before its transaction commits is
visible. Reported live: a dropped table stayed on screen for minutes because
the commit was durable in `_lgat` while the data volumes were untouched. The
delay a reader notices is CUBRID's flush cadence (`checkpoint_interval` default
360, `checkpoint_every_size` default 100000 log pages), not the follow debounce.
This is why the envelope carries `input_modified_unix_seconds` beside
`observed_at_unix_seconds`, and why the chip must show both.

**Only data volumes are watched.** `parse_vinf` skips `raw_id < 0`, so `_vinf`,
`_lginf`, `_bkvinf` and `_lgat` are never stat'd. This is correct — a missing
`_bkvinf` is common and must not break fingerprinting — and it is also the
mechanism behind the paragraph above. Do not add the log to the fingerprint to
"fix" staleness: on a busy database that publishes generation after generation
with identical facts.

**Enrichment never carries across generations.** `CONTEXT.md` states it
outright, which is why the browser re-issues the enrichment rather than the
server replaying it onto a new generation. Reproduced against a real database:
enrich a page, write to the volume, and the slot URL that answered a moment
earlier answers 404.

**Asset-contract tests are text assertions and have already lied.**
`browser_contract_uses_live_entity_canonical_history` greps `app.js` as a
string. Its predecessor stayed green across two commits while the server
abandoned the grammar it asserted, because it never spoke to a server. The
counterweight is `every_address_the_browser_builds_resolves` in `web.rs`, which
boots the adapter and asks for every address the browser builds. Extend that
test when adding browser routes; do not rely on the text assertion alone.

**Derive encoded lengths from constants.** `hex_decode` hard-coded 80 chars for
the old 8-byte cursor payload; widening the payload to 16 bytes silently broke
every cursor. Now `CURSOR_HEX_LEN` derives from `CURSOR_PAYLOAD_BYTES +
CURSOR_MAC_BYTES`.

**Prove causation in timing tests.** The first follow test passed in 30ms — less
than `poll + quiet + min_idle` could take — and would have passed just as
happily against a watcher that published on every poll. It now holds a quiet
input for twice `max_defer` and asserts generation 0 before writing. Keep that
guard.

## Verifying

```sh
just verify                      # fmt-check, test-debug, lint, elf-check-release
cargo test --lib web::tests      # the adapter, including the e2e socket tests
```

Test harness in `web.rs` `mod tests`: `boot(follow)` starts the adapter on
`127.0.0.1:0` over a synthetic one-volume fixture on its own runtime thread;
`get`, `shell`, `enrichment`, `exchange_raw` are a blocking HTTP client using
`Connection: close`; `brisk_follow()` is a short-fused `FollowConfig`.

Against a real database — `demodb` in the feat-oos install, 2 volumes,
192 sectors, 1464 OOS pages:

```sh
volmap serve --vinf /home/vimkim/.cub/db/feat-oos/commondb/demodb/demodb_vinf \
  --listen 127.0.0.1:7891
```

A full scan of that 192 MB database takes ~17 ms, so the load governor barely
engages. A write advances the generation in ~0.8 s, matching the 500 ms poll
plus 300 ms quiet period.

**Copy the volumes before mutating anything.** To exercise follow, copy
`demodb` and `demodb_x001` to a scratch directory, write a vinf naming the
copies, and mutate those. Rewriting a page onto itself moves the file stamp
without changing content, which is enough to trigger a re-read. Never write to
a database the user may still be using.
