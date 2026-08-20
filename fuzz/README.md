# LibFuzzer verification

This isolated workspace is not part of Volmap's runtime dependency graph. It
uses a separately locked `libfuzzer-sys` graph and exercises the
checked byte access, physical envelope, volume/bitmap, slotted record,
file/overflow/vacuum, and TDE key-info seams under LibFuzzer's sanitizer build.

Install the pinned driver and run the bounded smoke gate with:

```sh
cargo install --registry crates-io cargo-fuzz --version 0.13.2 --locked
rustup toolchain install nightly-2026-08-18 --profile minimal
fuzz/run-smoke.sh
```

`VOLMAP_FUZZ_RUNS` changes the per-target run count. The script seeds only from
the approved `fixtures/e1e651de` corpus; generated corpora and crash artifacts
remain untracked. Long-running corpus and additional discovery/request/export
targets remain separate release gates until those parsers expose bounded pure
seams.
