# Ticket 07: HTTP admission test race

The earlier local verification failure is reproduced and causally repaired in
`concurrent_http_scopes_share_scan_but_keep_coverage_and_disk_admission_independent`.
This changes test synchronization only. It adds no real-producer coverage and
leaves the external integration requirements open.

## Reproduction and diagnosis

Source baseline: `a40fcb1`. The existing test executable was run sequentially,
with a 15-second per-process timeout and the exact filter below. The original
failed on iteration 28 with “eight HTTP waiters must occupy admission”. A second
run with temporary caller status logging failed on iteration 11; epoch 7 returned
429 before the polling loop timed out. Raw logs are retained separately.

The empty HTTP polling request itself acquires admission. It can occupy the
eighth slot just as a valid caller arrives, rejecting that caller and leaving
only seven blocked callers. The poll then waits for eight callers that can no
longer exist. This is a test synchronization race, not evidence of a production
admission defect.

The repair submits nine valid requests and uses a bounded completion channel.
Before the gated producer responds, the first completion must be HTTP 429.
After checking that independent disk/session/capability endpoints still respond,
the producer is released and the remaining eight callers must each succeed.
All original per-caller scope, shared-capture, and partial-coverage assertions
remain. The refused caller identity is intentionally independent of scheduling.
Temporary logging was removed.

## Verification

The repaired exact test passed 500 consecutive executions, zero failures.
Build and select the same seam with:

```sh
cargo +1.97.1 test --locked --lib concurrent_http_scopes_share_scan_but_keep_coverage_and_disk_admission_independent
```

The repeat loop invokes the resulting lib test executable (the path is printed
by Cargo) with:

```sh
TEST_EXECUTABLE --exact web::tests::concurrent_http_scopes_share_scan_but_keep_coverage_and_disk_admission_independent --nocapture
```

Independent Standards and Spec reviews found no violations, false-pass paths,
or loss of the original contract. Severe scheduling delays can still exceed
the existing deadlines and fail; 500 passes are not a hard real-time guarantee.

The adjacent compressed logs retain every invocation count and exit status.
`test-change.patch` preserves the exact repair. Read logs with `gzip -dc` and
verify all artifacts with `sha256sum -c SHA256SUMS` from this directory.

The final `just verify` run passed, including Rust tests, Clippy, the static-musl
release gate, frontend checks and 47 browser passes with one existing skip.
`verify.log.gz` retains the full command output. Generated screenshots from
that run are archived separately; earlier committed evidence was restored.
