# Ticket 07 resumed: develop producer delivery

Integration is **incomplete**. This checkpoint resumes work item 126 from
`producer-08-handoff.md`; it does not close either release gate.

Producer source is `48a3e87e3d56d3b0c998d286f17fc6674bb5e547`, based on develop
`8cb558b3b264b5ed045ab24fa50b0ef825925349`. The engine changes are in `f426025bb`
and `4d9c394dd`; the final commit retains helper/evidence changes. Consumer
source is `7c6e7e9`; the pre-existing producer ticket edit was preserved.

## Independently repeated checks

- 43 source/build/installed binary hashes match the delivered manifest.
- All 151 producer07 artifact checksums and 126 retained producer06 artifact
  checksums pass. This validates retained evidence bytes, not new execution of
  native/credential/server/CTP tests.
- Both develop producer executables independently pass `[corpus]`: five
  top-level Catch cases, seven JUnit entries including section paths, and 1,256
  assertions per mode. Catch's JUnit `tests` attribute counts assertions here;
  it must not be reported as 1,256 test cases.
- The current consumer passes its corpus-pin test and all 28 observation unit
  tests, including the 39 canonical exchange fixtures, raw limits, real socket
  deadlines, identity/credentials, lifecycle and ambiguity checks.
- All 109 producer/consumer corpus files match. Version 1.0, revision 2,
  aggregate SHA-256
  `11dbecc72e4b9dd78e22080f138c801c189c7e047c0db23af8233f44a804d5ce`.
- Frontend typechecking and the full `just verify` gate pass: 74 frontend unit
  tests, 65 browser passes and one existing skip, plus Rust tests, Clippy and
  static-musl checks. Generated screenshots are retained in
  `verify-screenshots.tar.gz`; original tracked images were restored.

See [manifest.json](manifest.json) for named corpus entries, commands and source
pins; [provenance-recheck.json](provenance-recheck.json) records each expected
and observed hash. Logs and JUnit XML are retained beside them.

Reproduce the independent consumer checks from the Volmap root:

```sh
cargo +1.97.1 test --locked --test pgbuf_corpus -- --nocapture
cargo +1.97.1 test --locked --lib web::observations -- --nocapture
mise x node@24.19.0 -- corepack pnpm --dir web run typecheck
```

For each producer mode, select its matching installed libraries and execute
`BUILD/bin/test_pgbuf_inspector '[corpus]' --reporter junit`. Exact executable
paths are in the manifest; installations and library resolution are recorded in
producer07's `debug-identity.json` and `release-identity.json`.

## Delivered evidence superseding old missing-input notes

The producer07 ledger now supplies Debug and RelWithDebInfo (not plain Release)
results: two registered CTest entries, 61 inspector cases, seven native checks,
four exact-UID credential cases, 11 shipped-server PASS lines and one successful
external CTP case per mode. The external testcase revision is
`a648d78f599504fe0c916628ea51ea3f2b5f7ca7`.

The retained producer06 ledger separately establishes permanent VPID `1:577`:
a native controller retains a WRITE fix across clean/dirty observation and
acknowledges a cache miss after replacement. Actual Volmap HTTP consumes these
states; a byte-preserving pacing relay separately checks partial unknown and
one shared scan across eight callers. Its consumer is pinned at `5dacafb`.
This supplements the old temporary-page-only evidence; it is not a new run
against the current consumer or a controlled-state browser measurement.

Delivered evidence lives at
`/home/vimkim/gh/cb/CBRD-27398-pgbuf-inspector-develop/docs/pgbuf-inspector/verification/{06,07}`.
Both ledgers preserve failed and inconclusive attempts. Native tag checks prove
semantic page-kind translation, not valid persistent storage structures.

## Remaining boundaries

ADR0007 requires an explicit persistent format profile. The current `develop`
profile pins `cd593bc`; the delivered producer base is `8cb558b`. Volmap has
`e1e651de` and `e1e651de-records` disk fixtures but no independent develop disk
corpus. Synthetic ordinal tests and successful wire decoding cannot establish
this prerequisite. Generation scope is awaiting clarification; no develop
browser integration is claimed. The producer's `--volmap-run` helper explicitly
requires `feat-oos` and cannot be used as a develop entry point.

Windows/other Unix execution and Release/OptDebug configuration evidence remain
missing. Non-server symbol inspection is source/binary gating evidence, not
Windows execution. Dedicated-host performance and manual accessibility belong
to the final release gates and remain unproven.

## Current consumer against the format-aligned producer

The existing real-producer browser suite was rerun with current consumer
`7c6e7e9` and `feat-oos`. All 16 retained source/helper/build/installation hashes
checked against producer06 inputs match. Per-run runtime JSON records actual
binary and harness hashes, source state and copied-volume identities.

Debug passes all 12 Chromium/Firefox cases, zero failures/skips. The first
RelWithDebInfo attempt executes zero browser cases: Volmap cannot bind
`127.0.0.1:47622` (`Address in use`). After cleanup a standalone bind succeeds;
the owner at failure was not captured, so the specific transient collision
cause is unproven. Retain this failed attempt independently of any retry.
The retry selects checked ports 19622/19623 outside this host's automatic
32768–60999 range and uses a fresh output directory. No application or harness
code was changed, and no unrelated process was terminated.

The RelWithDebInfo retry passes all 12 cases, zero failures/skips/flaky results.
Together the fresh aligned matrix passes 24 browser cases. This remains a
shipped-server attachment/lifecycle/rendering gate; it does not establish the
missing independent develop disk corpus or controlled-state browser evidence.
