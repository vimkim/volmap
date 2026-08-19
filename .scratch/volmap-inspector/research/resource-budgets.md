# Resource-budget evidence and benchmark gate

## Conclusion

Ticket 17 cannot yet set evidence-backed numeric defaults for resident memory, spill capacity, worker count, OOS traversal steps, decoded bytes, scan throughput, or query latency. The implementation has advanced since the initial research pass: it now has snapshot discovery, a deterministic fast envelope scan, immutable in-memory revisions, explicit file-allocation traversal, cancellation checks, selective page decoding, and bounded OOS traversal. It still lacks the packed/spill store, worker pool, cursor query engine, immutable representative corpus, and the complete corruption/TDE matrix. Consequently the full workloads whose budgets govern version one still cannot run.

The only local database sample is also insufficient as a benchmark corpus: it is one small, mutable 192 MiB database, its current hashes differ from the hashes recorded in `provenance.toml`, and it does not supply the required medium, large, sparse, highly allocated, or corruption-heavy cases. Its geometry is useful corroborating metadata, not a measured basis for defaults.

This report therefore records the exact formulas supported now, withholds unsupported constants and performance claims, and defines the benchmark gate needed to finish Ticket 17. Supplying guessed defaults now would contradict the accepted requirement that resource exhaustion be observable as partial coverage rather than hidden sampling or truncation ([Ticket 10, resource safety](../issues/10-define-corruption-diagnostics.md#arithmetic-traversal-and-resource-safety)).

The CLI's current 256 MiB/2 GiB/four-worker/65,536-step values are development scaffolding, not accepted production defaults. They must be replaced or explicitly ratified by this ticket before a version-one release.

## Evidence scope

The authoritative format profile is CUBRID commit `e1e651debf6cc100172bde96603b17424f9c135a`, as pinned by [`provenance.toml`](../../../provenance.toml). Exact-commit source can be reproduced without depending on the newer worktree checkout:

```bash
git -C /home/vimkim/gh/cb/feat-oos show \
  e1e651debf6cc100172bde96603b17424f9c135a:src/storage/file_io.h
git -C /home/vimkim/gh/cb/feat-oos show \
  e1e651debf6cc100172bde96603b17424f9c135a:src/storage/storage_common.h
```

At that commit, `DISK_SECTOR_NPAGES` is 64 (`storage_common.h:109`), and the database page-type enum ends at ordinal 14 (`storage_common.h:148-167`). The current Rust profile fixes a 16,384-byte physical I/O page and a 16,344-byte database-page region ([`src/format/page.rs`](../../../src/format/page.rs)); the page-envelope decoder requires a complete 16,384-byte slice before it reads the duplicated envelope fields. These are format and current-interface facts, not scanner measurements.

The product architecture is already fixed independently of numeric defaults:

- ordinary fast-scan envelopes have 40 useful bytes, a 32-byte prefix plus an 8-byte trailing watermark;
- the store must use packed virtual page topology, compact facts, bounded resident memory, and private spill rather than one heap object per physical page;
- worker-local raw/decrypted page storage is bounded and never cached or spilled;
- budget refusal or exhaustion produces `inspection.resource_limit`, retained validated facts, truthful partial coverage, and a non-success outcome;
- no resource limit may silently sample, truncate, or claim complete coverage.

These constraints are defined in [Ticket 05](../issues/05-choose-scan-index-cache-architecture.md#initial-fast-inspection) and [Ticket 10](../issues/10-define-corruption-diagnostics.md#arithmetic-traversal-and-resource-safety). Ticket 05 itself explicitly delegates numeric defaults to representative measurement rather than inferring them from the development database ([resource and asymptotic contract](../issues/05-choose-scan-index-cache-architecture.md#resource-and-asymptotic-contract)).

## Reproducible observations

### Environment

The observations below were made at Volmap commit `944af198b47ebc0dc3bba2dd78081fbe275b7173` on Linux `6.19.10-300.fc44.x86_64`, x86-64, with Rust/Cargo `1.97.1`. The host reports 24 logical CPUs, 16,090,161,152 bytes of RAM, 8,589,930,496 bytes of swap, and an XFS filesystem on NVMe with 4,096-byte filesystem blocks. These describe this run only and do not justify defaults for constrained hosts.

Reproduce the environment capture with:

```bash
git -C /home/vimkim/temp/volmap rev-parse HEAD
rustc -Vv
cargo -V
uname -srmo
lscpu
free -b
findmnt -T /home/vimkim/.cub/db/feat-oos/commondb/demodb/demodb \
  -o TARGET,SOURCE,FSTYPE,OPTIONS
getconf PAGESIZE
```

### Available database metadata

The `_vinf` names two nonnegative data volumes. Their exact current metadata is:

| Volume | File bytes | 16 KiB pages | Current sectors | Maximum sectors | Allocated filesystem bytes | Extents |
|---|---:|---:|---:|---:|---:|---:|
| 0 | 67,108,864 | 4,096 | 64 | 64 | 67,108,864 | 2 |
| 1 | 134,217,728 | 8,192 | 128 | 512 | 134,217,728 | 2 |
| **Total** | **201,326,592** | **12,288** | **192** | — | **201,326,592** | **4** |

File size, allocation, and extent count are reproducible with:

```bash
snapshot=/home/vimkim/.cub/db/feat-oos/commondb/demodb
stat -c '%n %s %b %B' "$snapshot/demodb" "$snapshot/demodb_x001"
du -B1 "$snapshot/demodb" "$snapshot/demodb_x001"
filefrag -v "$snapshot/demodb" "$snapshot/demodb_x001"
```

The safe page-zero volume-header fields were read at their pinned offsets and agree with the 64-page sector geometry. They can also be decoded through the current page-envelope and volume-header modules, but there is no command that scans the database.

Reproduce the safe header-field and hash observations with:

```bash
snapshot=/home/vimkim/.cub/db/feat-oos/commondb/demodb
od -An -j 60 -N 2 -td2 "$snapshot/demodb"
od -An -j 72 -N 12 -td4 "$snapshot/demodb"
od -An -j 60 -N 2 -td2 "$snapshot/demodb_x001"
od -An -j 72 -N 12 -td4 "$snapshot/demodb_x001"
sha256sum "$snapshot/demodb" "$snapshot/demodb_x001" "$snapshot/demodb_vinf"
```

The current files are not immutable benchmark fixtures. Their hashes are:

```text
demodb       2ffe04d1cd9125d2e738f7d5384746c3e0e189c857ac8390cc27d1340889eaa2
demodb_x001  caaddd9bd4d2800eddd1b18b2f8bff3f3ea59a23c1dbf68192195a05478eb832
demodb_vinf  ad43f94dcf6d882ef041c6dc3f7bc05238aee09f33ef2364f6c9e40d4c1e5674
```

The first two differ from the historical hashes in [`provenance.toml`](../../../provenance.toml), so results against them are not repeatable acceptance evidence. Ticket 13 also requires an immutable, source-generated and annotated corpus before semantic decoders are considered complete ([acceptance matrix](../issues/13-prioritize-page-decoders.md#acceptance-matrix)).

### Initial executable capability (historical)

[`src/main.rs`](../../../src/main.rs) is intentionally a zero-interface executable. [`Cargo.toml`](../../../Cargo.toml) defines no runtime dependencies or benchmark target, and the source tree contains none of the scanner/store modules listed in the conclusion. The supported foundation test command is:

```bash
cargo test --release --locked
```

It passes 28 tests covering the current model, diagnostics, checked byte access, page envelope, volume header, and sector bitmap. This is a correctness control only. Cargo/test-runner wall time and RSS are not Volmap scanner throughput or resident-memory measurements and must not be used to set `ResourcePolicy`.

### Implementation checkpoint on 2026-08-19

The later implementation checkpoint scans the mutable 192 MiB sample as two volumes, 192 sectors, 12,288 physical pages, and 8,384 reserved-sector envelopes. Five warm-cache `summary --format json` invocations each completed below `/usr/bin/time`'s 0.01-second display resolution with 1,788–1,912 KiB maximum RSS. A deterministic full HTML export completed in 0.02 seconds with 29,136 KiB maximum RSS. These are useful smoke observations only: the sample is mutable, cache state was uncontrolled, the timer resolution is inadequate for percentiles, export materializes a large projection, and the packed/spill/worker/query paths do not exist.

The checkpoint release ELF is reported by `file` as static PIE and by `ldd` as statically linked. This validates the current build shape, not reproducibility or the distribution matrix. The authoritative tracker now reconciles 98 permanent file headers and all allocation tables; the resulting graph classifies 2,504 allocated, 5,876 reserved-unallocated, 3,904 unreserved, and 4 system pages. `map file:0:128` classifies three allocated pages in one sector. Real OOS head `oos:1:2243:0` validates two chunks and 30,728 payload bytes without retaining payload content.

The historical paragraphs below are retained to show why the benchmark gate was created. Statements that the executable cannot scan are superseded by this checkpoint; the missing-corpus and missing-path conclusions remain current.

## Supported formulas

Let `P` be all physical pages, `A` the system or allocated pages whose ordinary envelopes are inspected, `V` volumes, `S` sectors, `F` files, `C` compressed allocation claims, `D` committed deep records, `K` validated OOS chunks, `W` admitted workers, and `R` query rows. The accepted architecture supports the following statements before implementation:

| Quantity | Supported formula or bound | Status |
|---|---|---|
| Physical bytes represented | `16,384 * P` | Exact format arithmetic. |
| Useful ordinary-envelope bytes | `40 * A` | Exact logical field volume from the accepted scan contract. |
| Current decoder input | `16,384 * A` if invoked once per envelope | Exact for the current full-slice API; not the future source I/O plan. |
| Full-slice/useful ratio | `16,384 / 40 = 409.6` | Exact ratio, not observed filesystem amplification. |
| Minimum live full-page worker buffers | at least `16,384 * W` bytes | Lower bound only; queues, decoder state, allocator overhead, TDE buffers, and graph state are unmeasured. |
| Fast work | `Theta(V + S + F + A)` plus bounded sorting of `C` claims | Accepted asymptotic contract, not a latency prediction. |
| Spill growth | `O(V + S + F + C + A + D)` encoded safe facts | Record constants and worst-case diagnostic/evidence density are unimplemented and unknown. |
| One deep page | one 16 KiB page read plus decoder work | Exact request granularity; decoder costs are unmeasured. |
| One OOS chain | `Theta(K)` page/slot work and `O(K)` visited identities | Requires both step and decoded-byte limits; no safe default follows from asymptotics alone. |
| Paged query | `O(log N + R)`, or `O(R)` after cursor seek | Accepted target; the query/index implementation does not exist. |

For the one available database, `P = 12,288`. Even if every page were eligible (`A = P`), the future logical envelope field volume would be only `491,520` bytes. The current decoder's full-slice input would be `201,326,592` bytes. This 409.6 ratio must not be called measured I/O amplification: actual amplification depends on the unimplemented positional-read batching strategy, filesystem cache state, device block behavior, and whether adjacent reads coalesce.

## Required benchmark matrix

All cells below are required before numeric defaults or expectations are released. “Missing” means the current source cannot execute the measurement, not that the value is zero.

| Corpus case | Required controlled property | Corpus now | Fast scan | Peak resident | Peak spill | Cancellation/limit semantics | Query p50/p95/p99 | OOS step/byte boundary |
|---|---|---|---|---|---|---|---|---|
| Small | pinned immutable baseline, fully annotated | One mutable 192 MiB corroboration sample only | Missing | Missing | Missing | Missing | Missing | Missing |
| Medium | multi-volume, mixed allocation and page types | Missing | Missing | Missing | Missing | Missing | Missing | Missing |
| Large | enough pages/claims to force external spill | Missing | Missing | Missing | Missing | Missing | Missing | Missing |
| Sparse | large geometry, few allocated pages | Missing | Missing | Missing | Missing | Missing | Missing | Missing |
| Highly allocated | dense allocation and envelope facts | Missing | Missing | Missing | Missing | Missing | Missing | Missing |
| Corruption-heavy | deterministic independent and cross-entity findings | Missing | Missing | Missing | Missing | Missing | Missing | Missing |
| OOS boundary | 1, exact-boundary, multi-page, cyclic, and over-budget chains | Generation recipes exist, immutable corpus missing | N/A | Missing | Missing | Missing | N/A | Missing |

Each generated case must record the exact pinned engine commit, build/profile configuration, creation commands, clean-stop procedure, hashes, sizes, page/sector/file counts, allocation density, page-type counts, expected findings, and annotated OOS chains. Corrupt variants must mutate copies at named evidence locations and retain independent expected outcomes.

For each case and each candidate policy, run at least:

1. cold-cache and warm-cache fast scans with one worker and each candidate worker count;
2. resident limits immediately below, at, and above each segment-flush/admission boundary;
3. spill limits immediately below, at, and above the required encoded graph size;
4. cancellation at every safe publication boundary and during spill merge;
5. overview, lookup, first-page enumeration, cursor continuation, diagnostic-heavy enumeration, and maximum 512-row queries;
6. OOS traversal at `limit - 1`, `limit`, and `limit + 1` for both steps and decoded bytes; and
7. repeated runs sufficient to report sample count, median, p95/p99 where applicable, minimum/maximum, and variance rather than a single favorable run.

Record useful logical bytes, requested syscall bytes, actual block I/O where available, elapsed and CPU time, peak RSS, allocator-resident bytes, bytes in each graph segment, peak/total spill, worker occupancy, queue/admission refusals, facts/diagnostics/evidence emitted, and coverage/outcome. Verify output equivalence across worker counts and memory/spill regimes.

## Numeric-default decision

| CLI control or expectation | Evidence-backed value now | Decision |
|---|---:|---|
| `--memory-limit` | none | Withhold until packed-store and worst-case diagnostic/evidence measurements exist. |
| `--spill-limit` | none | Withhold until the large/dense/corrupt matrices force and measure spill. |
| `--workers` | none | Withhold until cold/warm scaling and per-worker memory are measured on constrained and reference hosts. |
| `--max-chain-steps` | none | Withhold until pinned OOS fixtures cover valid long chains and hostile cycles. |
| `--max-decoded-bytes` | none | Withhold until semantic decoders account decoded input consistently and boundary fixtures exist. |
| Fast-scan throughput/latency | none | Publish no expectation from a crate that cannot scan. |
| Query latency | none | Publish no expectation before the packed indexes and cursor queries exist. |
| Filesystem I/O amplification | none | Preserve `40 * A` as useful-byte accounting; measure requested and device bytes separately. |

The only safe interim operational choice during implementation is explicit test-supplied policies, not a public default. Tests may use deliberately tiny limits to prove admission, spill, cancellation, `inspection.resource_limit`, partial coverage, deterministic output, and cleanup. A version-one default becomes supportable only after the matrix above passes on at least one constrained host and one reference host and every output remains semantically identical across budget regimes except for the explicitly expected partial result at a reached limit.
