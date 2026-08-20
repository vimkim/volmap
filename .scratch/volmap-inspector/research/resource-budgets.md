# Resource-budget measurements and version-one defaults

## Conclusion

Ticket 17 now has executable measurements for every implemented resource path.
Commit `2c19cca` adds deterministic small, medium, large, sparse, dense,
corruption-heavy, resident, spilled, cancellation, query, and OOS-boundary
profiles. Thirty warm-cache samples per fast-scan cell passed with identical
inspection output across one/four workers and resident/spilled storage.

The accepted internal version-one defaults are:

| Control | Default | Reason |
|---|---:|---|
| `--memory-limit` | 256 MiB | Hard admission cap, not a preallocation. It holds 16,777,216 exact 16-byte facts before fixed topology/diagnostic charges and otherwise spills. |
| `--spill-limit` | 2 GiB | Holds 134,217,728 facts, equivalent to 2 TiB of fully eligible 16 KiB pages, while remaining an explicit finite disk boundary. |
| `--workers` | 4 | Four-way scans improved the large/dense/corrupt medians by 1.46–1.66× on the reference host with deterministic merge order. |
| `--max-chain-steps` | 16,384 | At one physical page per step this meets the 256 MiB decoded-byte boundary exactly; a larger independent default cannot admit more OOS pages. |
| `--max-decoded-bytes` | 256 MiB | The 512-page/8 MiB OOS matrix passed exact byte boundaries; the default supplies 32× measured headroom while remaining finite. |

These limits preserve the accepted semantics: non-admitted work does not start,
resource exhaustion publishes a validated prefix with `partial` coverage and an
`inspection.resource_limit` diagnostic, and no path silently samples or claims
complete coverage.

The measurements support internal defaults, not a universal throughput SLO.
The run is warm-cache on one reference host. Controlled cold-cache, a physically
constrained host, and the supported Linux distribution matrix remain release
checks.

## Reproduction

The matrix is an ignored release-mode integration benchmark so ordinary tests
compile it without spending benchmark time:

```bash
git checkout 2c19cca
VOLMAP_BENCH_SCALE=full VOLMAP_BENCH_SAMPLES=30 \
  cargo test --release --locked \
  --test resource_benchmark -- --ignored --nocapture
```

`just resource-benchmark` is the equivalent local convenience command. It is
not presented as CUBRID organization workflow.

The raw captured test output had SHA-256
`4c0ae07ecb3316fd2cfbe2d29332e96dcd990e1b4bab6ac69cf202212b210a8f`.
Every result is emitted as one JSON object so later runs can compare fields
without parsing prose.

### Reference environment

- Linux `6.19.10-300.fc44.x86_64`, x86-64
- Intel Core Ultra 7 270K Plus, 24 logical CPUs
- 16,090,161,152 bytes RAM
- Rust `1.97.1` (`8bab26f4f68e0e26f0bb7960be334d5b520ea452`)
- static `x86_64-unknown-linux-musl` test executable
- XFS/NVMe workspace

The direct test executable reported 1,796 KiB peak RSS in a three-sample full
run. The thirty-sample in-process `VmHWM` reached 2,964 KiB. Timing the outer
`cargo test` is not a product RSS measure because a necessary rebuild can add
the compiler's memory.

## Generated profile matrix

The generator uses the pinned 16 KiB envelope, volume-header, bitmap, and OOS
layouts. It creates new files per run, records logical and allocated bytes, and
removes them afterward. Dense pages are materialized; sparse geometry uses a
real sparse file. Corruption is a deterministic zero-envelope mutation at every
seventh eligible page.

| Profile | Logical size | Allocated size | Physical pages | Eligible envelopes | Purpose |
|---|---:|---:|---:|---:|---|
| small | 64 MiB | 1 MiB | 4,096 | 64 | startup/low-count baseline |
| medium | 256 MiB | 64 MiB | 16,384 | 4,096 | mixed logical/allocation scale |
| large | 1 GiB | 256 MiB | 65,536 | 16,384 | resident/spill comparison |
| sparse | 4 GiB | 1 MiB | 262,144 | 64 | prove work follows reservation, not logical size |
| dense | 512 MiB | 512 MiB | 32,768 | 32,768 | maximum measured envelope density |
| corrupt | 128 MiB | 128 MiB | 8,192 | 8,192 | deterministic diagnostic growth |
| OOS | 64 MiB | about 9 MiB | 4,096 | 576 | 512-chunk complete and cyclic chains |

The profiles are structural performance inputs, not replacements for the
source-derived semantic fixture corpus. The source generator itself and the
pinned layout tests are the reproducible authority.

## Fast-scan results

Times are milliseconds for 30 warm-cache process-internal samples. `p99` is the
largest observation with this sample count.

| Profile | Store | Workers | p50 | p95 | p99 |
|---|---|---:|---:|---:|---:|
| small | resident | 1 | 0.142 | 0.421 | 0.497 |
| small | resident | 4 | 0.119 | 0.239 | 0.302 |
| medium | resident | 1 | 6.911 | 7.528 | 7.530 |
| medium | resident | 4 | 4.786 | 7.684 | 9.817 |
| large | resident | 1 | 27.878 | 28.846 | 29.993 |
| large | resident | 4 | 17.654 | 19.484 | 20.282 |
| large | spill | 4 | 20.861 | 21.516 | 21.676 |
| sparse | resident | 1 | 0.139 | 0.175 | 0.190 |
| sparse | resident | 4 | 0.085 | 0.152 | 0.185 |
| dense | resident | 1 | 57.915 | 60.792 | 61.082 |
| dense | resident | 4 | 34.979 | 37.884 | 39.488 |
| dense | spill | 4 | 41.341 | 42.939 | 43.290 |
| corrupt | resident | 1 | 13.109 | 13.920 | 14.693 |
| corrupt | resident | 4 | 8.865 | 9.799 | 10.747 |

On the reference host, the largest resident/spill cells sustained roughly
0.76–0.94 million eligible envelopes per second. This is an observation, not a
minimum guarantee.

### Exact accounting

Fast scan does not read a full 16 KiB page. `VolumeHandle::read_envelope` issues
one positional read for the 32-byte prefix and one for the 8-byte watermark.
Therefore:

- requested envelope bytes are exactly `40 * A`;
- requested envelope read calls are exactly `2 * A`;
- packed fact bytes are exactly `16 * published facts`;
- resident fact admission is `16 * A` plus fixed topology, diagnostic, and
  terminal-diagnostic reserve charges;
- spill growth is exactly 16 bytes per published fast fact.

Actual block-device amplification remains cache/filesystem/device dependent and
must not be inferred from requested bytes.

| Measured case | Admitted resident | Spill | Requested envelope bytes |
|---|---:|---:|---:|
| large resident | 262,864 B | 0 | 655,360 B |
| large forced spill | 720 B | 262,144 B | 655,360 B |
| dense resident | 524,944 B | 0 | 1,310,720 B |
| dense forced spill | 656 B | 524,288 B | 1,310,720 B |
| corruption-heavy | 387,753 B | 0 | 327,680 B |

The resident and forced-spill graph overviews were equal at both large and
dense scales. The corruption profile retained every admitted finding under the
default cap; separate ordinary tests prove a smaller diagnostic cap stops with
explicit partial coverage.

## Query, cancellation, and OOS results

Twenty thousand warm queries against the dense resident graph produced:

| Operation | p50 | p95 | p99 | Maximum |
|---|---:|---:|---:|---:|
| overview | 88 ns | 94 ns | 96 ns | 1,922 ns |
| 64-page sector projection | 2,044 ns | 2,201 ns | 3,071 ns | 144,259 ns |
| page lookup | 28 ns | 29 ns | 31 ns | 104 ns |

These are core in-process operations, not HTTP/terminal end-to-end latency and
not a cursor-engine claim.

Cancellation requested after 1,024 published envelopes returned an incomplete
revision in 4.122 ms. Exactly 1,024 facts were published; 1,088 envelope reads
were admitted, bounding read-ahead to the current four-worker × 16-page wave.

The 512-chunk OOS matrix proved both sides of each boundary:

| Case | Published chunks | Complete | Diagnostic | Elapsed |
|---|---:|---|---|---:|
| step limit 511 | 511 | no | `resource-limit` | 0.792 ms |
| step limit 512 | 512 | yes | none | 1.658 ms |
| byte limit 511 pages | 511 | no | `resource-limit` | 1.552 ms |
| byte limit 512 pages | 512 | yes | none | 1.513 ms |
| 512-page cycle | 512 | no | `oos.chain.acyclic` | 1.743 ms |

## Remaining release checks

The following are not reasons to withhold the internal defaults, but remain
mandatory before a public version-one performance/support claim:

- controlled file-specific cold-cache runs with requested and device I/O;
- the same matrix on a physically constrained host;
- Debian/Ubuntu, Rocky/RHEL-compatible, and Alpine execution;
- larger engine-generated semantic/OOS/TDE corpora and long-running fuzz jobs;
- end-to-end HTTP and terminal latency rather than core query calls; and
- written company public-release approval.
