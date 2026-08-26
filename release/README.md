# Release audit

The source tree pins its complete Cargo dependency graph in `Cargo.lock` and
its frontend graph in `web/pnpm-lock.yaml`. Cargo downloads missing crate
sources from crates.io into its normal shared cache; pnpm installs only from
the reviewed npm registry with dependency lifecycle scripts disabled. Locked
commands prevent either graph from resolving silently.

Release metadata is generated with exactly:

- `cargo-about 0.9.2`
- `cargo-cyclonedx 0.5.9`
- `cargo-deny 0.20.2`

The live-viewer foundation is generated with exactly Node 24.19.0, pnpm
11.24.0, and the package versions in `web/package.json`. Playwright 1.62.1 pins
Chromium 151.0.7922.34 revision 1234 and Firefox 153.0 revision 1538 in
`web/toolchain.json`.

After a reviewed dependency change, install those pinned tools and run
`release/regenerate-artifacts.sh`. Review the generated notice, SBOM, policy
result, `Cargo.lock`, build scripts, unsafe/native-code surface, and licenses
before committing them. `release/regenerate-frontend.sh` regenerates only the
committed browser assets, runtime notices/SBOM, and full build provenance.
Ordinary Cargo builds do not run that script and do not require Node.

`release/check-frontend.sh` performs an immutable install, type and unit checks,
advisory audit, deterministic bundle/evidence comparison, real-server Chromium
and Firefox tests, and a Cargo build with failing Node/pnpm shims. On a clean
candidate commit, `release/check.sh` also regenerates the frontend in two
different absolute paths—the second install is offline from the first one's
content-addressed store—then compares assets and final binaries alongside the
Cargo policy, notices, combined artifact SBOM, tests, and static-musl ELF gates.
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
