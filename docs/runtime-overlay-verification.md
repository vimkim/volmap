# Verify an actual CUBRID producer

The ordinary browser suite uses `producer-fixture.py`, a scripted Unix-socket
peer. Keep that deterministic coverage, and run the separate real-producer suite
when a matching CUBRID installation is available. Neither suite replaces the
other. `just verify` does not silently run or claim the external engine gate.

The real suite creates a disposable database, starts the installed `cub_server`,
and attaches actual Volmap processes to the original and copied permanent
volumes. It runs Chromium and Firefox against the embedded viewer without
intercepting HTTP responses or substituting the producer protocol. Six cases per
browser check selected-page observation and sanitized no-store responses,
copied-volume refusal with working disk navigation, restart while paused and
explicit retry, Volume/Sector state marks and LRU topology, and independent
pause/resume across tabs.

The fixture first starts the server with `enable_pgbuf_inspector` omitted and
requires successful SQL plus no inspector directory. It then restarts with the
parameter enabled. This checks the actual startup default, independently of the
older producer driver's explicitly disabled startup case.

## Inputs and execution

Prepare matching CUBRID source, build and installed binaries, plus a built Volmap
binary and the pinned frontend dependencies/browsers. CUBRID engine builds and
the companion shell testcase belong to their producer workflow. In Volmap,
`just build-debug` or `just build-release` produces the consumer binary;
`just frontend-install-browsers` prepares the browser toolchain.

Create a run JSON with explicit absolute paths and two unused loopback ports:

```json
{
  "producer_source": "/path/to/CUBRID-source",
  "producer_build": "/path/to/CUBRID-build",
  "producer_install": "/path/to/CUBRID-install",
  "consumer_binary": "/path/to/volmap/target/x86_64-unknown-linux-musl/debug/volmap",
  "format_profile": "feat-oos",
  "listen_port": 47150,
  "copy_listen_port": 47151,
  "output_directory": "/path/to/evidence/debug-attempt-001"
}
```

For the `e1e651d`-based producer, select `feat-oos`. Use separate JSON files and
fresh output directories for debug and release, choosing the corresponding
build/install/consumer paths. The helper refuses an existing output directory,
a mismatched producer/consumer corpus tree, or invalid ports. It does not reuse
an unrelated server listening on the requested port. Missing inputs fail instead
of silently skipping the engine checks.

From the Volmap root:

```sh
VOLMAP_PRODUCER_RUN=/absolute/path/debug-run.json \
  mise x node@24.19.0 -- corepack pnpm --dir web exec playwright test \
  --config playwright.producer.config.ts
```

`output_directory` contains the actual utility/process commands, exit codes,
disposable database root, copied-file identities, repository revisions and dirty
state, binary hashes, build type, corpus identity, and harness hashes. Adjacent
`-browser-results.json` and `-browser-artifacts/` paths contain named case results,
selected observations, screenshots and failure traces. Require all 12 cases,
zero failures and zero skips; a filtered or empty run is not the complete gate.
The helper shuts down only its own Volmap processes and private CUBRID registry.
Its disposable database root follows Python's `TMPDIR` convention; set that
variable for the real-producer run when `/tmp` lacks space. The path must remain
short enough for Unix sockets. Do not apply a private-home `TMPDIR` to the
separate credential-isolation suite, whose mapped UIDs need a traversable parent.
It retains the disposable files for failure investigation. Browser lifecycle
commands write to files because the daemon inherits stdout; waiting for a pipe's
EOF would incorrectly wait for the daemon itself to terminate.

## Evidence limits

These are real attachment/render/lifecycle checks, not independent proofs of a
held dirty page or eviction. Page `0:0` is observed in the small startup fixture;
its state is not controlled by a native acknowledgement. Keep the producer's
bounded native-fixture checks alongside this suite. That fixture's temporary
VPID `32766:65` does not establish rendering of a permanent inspected page.

Keep independent offline corpus tests, socket/credential and budget-boundary
tests, the companion CTP shell results, and scripted cancellation, timeout,
partial/malformed-stream and hidden-tab cases. Preserve failed and inconclusive
attempts. Recorded binary/source paths and hashes identify the inputs; they are
not a substitute for establishing how an installation was built.

The LRU cases are rendering smoke checks: classes, controls and limitation labels
are checked against real observations. They do not independently prove native
list membership, count accuracy, or coherent latch/LRU tuple sampling.

The develop producer port is a separate delivery and debug/release gate. Passing
wire or startup-fixture checks does not establish broad develop disk-format
support. Select the explicit persistent profile specified by ADR 0007 and retain
the independent format corpus evidence. Ticket 07 stays open while any required
cross-repo invariant or external build/test evidence is missing.

## Develop delivery checkpoint

The develop producer is now delivered at `48a3e87e3`, based on `8cb558b3b`,
with Debug and RelWithDebInfo native, socket, credential, lifecycle and external
CTP evidence. The [resumed ticket 07 ledger](../.scratch/pgbuf-overlay-implementation/verification/07-develop/README.md)
records the independently repeated corpus checks and exact input revalidation.

Volmap's explicit `develop` profile pins `cd593bc`. The independent
[develop disk corpus](../fixtures/8cb558b3/README.md) now records real pages from
the delivered `8cb558b3`-based engine and source-layout compatibility for the
central structures tested. It exposed and now guards the profile-specific heap
header size/statistics offsets and file-header boundary. The
[new disk/integration ledger](../.scratch/pgbuf-overlay-implementation/verification/07-develop-disk/README.md)
records affected reruns. This bounded corpus does not claim every develop disk
layout. The producer's native `--volmap-run` path still requires `feat-oos`.

The delivered producer06 evidence also supplies a controlled permanent VPID
`1:577` through actual Volmap HTTP at consumer `5dacafb`, with independent
clean/dirty/eviction acknowledgements. This supplements the temporary VPID
qualification above; it does not turn this browser suite's startup-page and
LRU smoke checks into controlled-state browser evidence.

Ticket 07's final matrix passes all 48 browser cases across both source profiles
and Debug/RelWithDebInfo. CUBRID's project `release` mode selects RelWithDebInfo.
The [final requirement audit](../.scratch/pgbuf-overlay-implementation/verification/07-develop-disk/completion-audit.md)
and independent reviews support functional verification completion. This does
not establish ticket 08 performance, manual accessibility or overall release
readiness, and it does not claim execution on Windows or every Unix platform.
