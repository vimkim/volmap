# Develop disk verification and actual producer integration

Ticket 07 and work item 126 are **complete** for the required functional
verification, following independent Standards and Spec review. Whole-feature
release readiness remains separate.

This continues work item 126 after `15acca5`. The earlier scope question was
unnecessary: the requested verification already includes the prerequisites for
validating the delivered develop producer. This directory records the new disk
corpus, defects it exposed and their affected integration reruns.

## Independent disk evidence

[Corpus](../../../../fixtures/8cb558b3/README.md): 19 engine-generated pages from
two volumes, pinned to producer `48a3e87e3` / base `8cb558b3`. The generator is
independent of Volmap. Source definitions checked against profile pin `cd593bc`
are identical; native Debug type information independently confirms field sizes
and offsets. `source-layout-audit.json`, `native-heap-layout.log.gz` and
`native-file-layout.log.gz` retain that evidence.

The new regression first fails with `heap.page.role_length`: develop uses a
1152-byte heap header without the OOS VFID at offset 32. Selecting the profile's
size and statistics offsets fixes it. The expanded corpus then fails with
`file.header.partial_table`: native develop tables start at offset 200, while
the old check always requires 216. Selecting the profile's minimum header size
fixes this second defect while preserving the stricter other-profile boundary.

`heap-red.log.gz` and `disk-corpus-02.log.gz` retain the actual decoder failures.
`disk-corpus-green.log.gz` contains six passing develop cases and six passing
format-aligned cases. A subsequent wrong-profile file-header assertion supplements
the existing heap assertion and runs in the final full gate.

The full stable dataset also passes actual CLI summary and selected deep page
inspection (heap header, file header, catalog and overflow). Every concrete
native page-type count matches Volmap's independently decoded count; there are
no diagnostics and the snapshot is valid. `success-limited` records selective
deep enrichment, not a failure or a claim of complete database semantic coverage.
See `disk-summary.json`, `disk-crosscheck.json` and `inspect-*.json`.

## Retained unsuccessful attempts

- `generation-01-rejected.tar.gz`: the first extraction script had an incorrect
  manually transcribed native ordinal table. No consumer test accepted that
  corpus. Checking the actual pinned PAGE_TYPE declaration corrected the table;
  the accepted second generation is under `fixtures/8cb558b3`, with a separate
  source database root and fresh hashes.
- `disk-corpus-01.log.gz`: test compilation rejected LowerHex formatting of the
  digest array. Formatting individual digest bytes fixed the test compilation;
  this is not a runtime pass.
- The first summary command combined JSON with the human-only `--diagnostics`
  flag and was refused with exit 2. The recorded accepted command omits that flag.

## Reproduction

Run the corpus command above independently of the producer's semantic corpus.
For the full dataset, use the `volumes[].path` parent from its manifest:

```sh
volmap summary --vinf ROOT/volmap_develop_vinf --volume-root ROOT \
  --format-profile develop --progress never --format json
volmap inspect page:0:129 --vinf ROOT/volmap_develop_vinf --volume-root ROOT \
  --format-profile develop --progress never --format json
```

Run the existing real-producer browser config using each retained `*-run.json`:

```sh
VOLMAP_PRODUCER_RUN=/absolute/path/debug-run.json \
  mise x node@24.19.0 -- corepack pnpm --dir web exec playwright test \
  --config playwright.producer.config.ts
```

Inputs name explicit source/build/install/consumer paths and checked ports
outside this host's ephemeral allocation range. Each run must use a fresh output
directory. The shipped server and browser harness remain unmodified. The native
`--volmap-run` driver remains restricted to `feat-oos`; its reruns here verify the
controlled permanent target against the changed consumer, not develop disk data.

The unchanged database commands can require substantially more space than their
requested volume sizes. The first develop release and final aligned debug runs
failed at createdb with insufficient `/tmp` space; zero browser cases executed.
The browser helper now follows Python's `TMPDIR` convention. Accepted reruns use
`TMPDIR=/home/vimkim/tmp` and short private roots. This changes only fixture
placement; it adds no producer control or observation behavior.

An attempted full gate with the same private-home TMPDIR failed the separate
mapped-UID credential test: its child cannot traverse the private parent. The
full gate therefore uses ordinary `/tmp`, preserving exact-UID checks. No
credential rule or directory permission was relaxed. Three stopped database
roots created by this turn were copied to the roomy filesystem, verified by
file SHA-256, then removed from `/tmp`. `database-relocations.json` binds original
and retained paths and records the inactive socket names excluded from the copy.
The first attempted cross-filesystem move stopped on an inactive SP socket;
subsequent copies verified every regular file before removing the originals.

Source and binary provenance from producer07 remain valid: no engine source,
build or installation changed. The new tests affect only Volmap disk decoding
and the external browser harness's temporary-root placement. Earlier corpus,
credential and producer native results are not relabelled as new executions.

## Accepted final matrix

| Gate | Debug | RelWithDebInfo / optimized consumer |
| --- | --- | --- |
| Develop shipped-server Chromium + Firefox | 12 passed | 12 passed |
| Format-aligned shipped-server Chromium + Firefox | 12 passed | 12 passed |
| Controlled permanent native target through actual HTTP | 11 PASS lines | 11 PASS lines |

All four final browser runs use the TMPDIR-aware helper, execute all 12 cases,
and have zero failures/skips/flaky cases. The accepted full `just verify` gate
also passes: Rust, Clippy, static-musl checks, 74 frontend unit cases and 65
browser cases plus one intentional Chromium-owned parity skip in Firefox.
`manifest.json` identifies the accepted reports separately from earlier attempts.

The [requirement audit](completion-audit.md) maps all eleven ticket criteria to
evidence and distinguishes them from broader release-readiness obligations.
Independent Spec review confirms that all eleven requirements are satisfied
within the documented Linux matrix. Standards and Spec each report zero
findings; see [review.md](review.md).
