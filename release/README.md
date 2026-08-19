# Release audit

The source tree carries its complete locked Cargo graph in `vendor/`. Normal
builds are offline through `.cargo/config.toml`; a missing or altered vendored
crate is fatal.

Release metadata is generated with exactly:

- `cargo-about 0.9.2`
- `cargo-cyclonedx 0.5.9`
- `cargo-deny 0.20.2`

After a reviewed dependency change, install those pinned tools and run
`release/regenerate-artifacts.sh`. Review the generated notice, SBOM, policy
result, `Cargo.lock`, vendored sources, build scripts, unsafe/native-code
surface, and licenses before committing them.

On a clean candidate commit, `release/check.sh` regenerates and compares both
artifacts, runs all dependency policy checks, extracts the commit into two
different absolute paths, builds and tests without network access, proves the
two release binaries are byte-identical, and verifies the static musl ELF.
The fixed epoch is the pinned CUBRID format-authority commit time. Path remaps
remove checkout and Cargo-home locations from compiled artifacts.

Passing this local audit does not authorize public distribution. Cross-distro
execution is checked separately with `release/check-distributions.sh`. That
gate runs the same static binary, with runtime networking disabled, through
canonical summary/map/page/export operations on a deterministic sparse smoke
snapshot in digest-pinned Debian 13, Rocky 9, and Alpine 3.24 containers and
requires byte-identical projections.

The complete fixture/fuzz/resource matrix, source and binary bundle checksums,
and written CUBRID ownership/legal approval remain mandatory gates in the
implementation specification.
