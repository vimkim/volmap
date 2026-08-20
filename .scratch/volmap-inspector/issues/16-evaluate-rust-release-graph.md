Type: research
Status: resolved
Blocked by: 01, 05, 06, 07, 08, 09, 13, 14, 15

# Evaluate the final Rust dependency and reproducible-release graph

## Question

Once the parser architecture and interface prototypes have selected their actual capabilities, what exact pinned Rust toolchain, crates, features, native-code exclusions, embedded assets, dependency-source strategy, and release process should Volmap Inspector use? Produce a lockfile-level recommendation that proves `x86_64-unknown-linux-musl` static linkage, reproducible construction, minimal attack and license surface, required Apache-2.0/CUBRID and third-party notice delivery, an SBOM, and identical behavior across CLI, TUI, HTML export, and web-service modes. Build-time network access is allowed; binary mobility must be proved from the final ELF and cross-distribution execution. Identify any dependency that introduces C/C++, shared libraries, build scripts, TLS, reciprocal licensing, or unreproducible inputs, and recommend an evidence-backed replacement or explicit approval gate.

## Comments

### Standing human disposition

On 2026-08-19 the user directed every remaining ticket to accept the source-backed recommended option and continue without further HITL. The exact graph below is therefore accepted after lockfile, target, license, build-script, and ELF audits rather than deferred for another approval round.

### Evaluated graph

The implementation capabilities selected by the resolved tickets require command parsing, deterministic JSON, an interactive terminal, HTTP/1.1 routing, hashing, constant-time bearer-token comparison, and AES/ARIA-256-CTR with zeroization. The repository now records the exact direct dependencies and feature sets in `Cargo.toml` and the complete transitive resolution/checksums in Cargo lockfile version 4.

The accepted direct graph is:

| Crate | Exact version | Enabled features / defaults | Purpose |
|---|---:|---|---|
| `serde` | 1.0.229 | defaults + `derive` | canonical projection types |
| `serde_json` | 1.0.151 | defaults | deterministic JSON/JSONL serialization |
| `clap` | 4.6.6 | defaults off; `derive,error-context,help,std,suggestions,usage` | explicit CLI contract |
| `tokio` | 1.53.1 | `net,rt-multi-thread,signal,sync,time` | bounded HTTP runtime and shutdown |
| `axum` | 0.8.9 | defaults off; `http1,json,query,tokio` | same-origin HTTP/1.1 adapter |
| `crossterm` | 0.29.0 | defaults off; `events` | terminal I/O, mouse, resize, keyboard |
| `sha2` | 0.11.0 | defaults off | snapshot and artifact SHA-256 |
| `subtle` | 2.6.1 | defaults off | constant-time token comparison |
| `aes` | 0.9.2 | `zeroize` | AES-256 primitive |
| `aria` | 0.2.0 | `zeroize` | ARIA-256 primitive |
| `cipher` | 0.5.2 | `zeroize` | pinned block/stream cipher traits |
| `ctr` | 0.10.1 | `zeroize` | pinned CTR construction |
| `zeroize` | 1.9.0 | defaults + `std` | secret-container destruction |
| `if-addrs` | 0.15.0 | defaults | enumerate active local interface addresses for copyable `serve` URLs |

Ratatui 0.30.2 was evaluated and rejected. Even with defaults disabled it added roughly fifty locked packages and extensive layout/widget/procedural-macro machinery. The accepted terminal prototype needs one adaptive layout, not a general widget framework, so a deep terminal-rendering module over Crossterm gives greater locality with a smaller interface and release graph. Memory mapping, Rayon, a database, compression, template engines, asset pipelines, logging frameworks, TLS stacks, regex, general error crates, and CUBRID libraries are likewise excluded until a measured requirement justifies them.

### Audit evidence

On 2026-08-20, Rust 1.97.1 (`rustc` commit `8bab26f4f68e0e26f0bb7960be334d5b520ea452`) resolved 89 lockfile packages and 85 packages in the Linux musl target tree, including Volmap. `cargo test --workspace --all-targets --release --locked` passed the complete test suite.

No resolved package has Cargo's `links` field. No active feature compiles C or C++ source, invokes CMake/pkg-config, or links a third-party shared library. Eleven packages contain build scripts; ten perform Rust configuration/probing only. `signal-hook` contains an optional `cc` path for `extended-siginfo-raw`, but that feature is disabled and its active build script is empty. `if-addrs`, `libc`, `rustix`, `mio`, and related packages call the musl/kernel ABI from Rust; they do not introduce a runtime shared-library dependency. The target tree contains no OpenSSL, native-tls, rustls, ring, bindgen, cc, or CMake package.

Every declared license is permissive: Apache-2.0, MIT, BSD-3-Clause, BSL-1.0, Unicode-3.0, Unlicense, or the LLVM exception in allowed alternatives/combinations. No reciprocal license is present. The release license policy must nevertheless deny missing/unlicensed, AGPL/GPL/LGPL/MPL/EPL/CDDL/SSPL, unknown git sources, unapproved registries, and unreviewed license-file exceptions; additions to the small non-MIT/Apache set require an explicit notice audit.

Two independent locked release builds used `SOURCE_DATE_EPOCH=1776342420`, path remapping, and separate target directories. They were byte-identical with SHA-256 `144813062afde5b1f2952311fa5b37cf988db668c7407607ff0a0f3a1e111c8b` for the current empty-command binary. `file` reported a stripped x86-64 static PIE; `readelf` reported no `NEEDED` entry; `ldd` reported `statically linked`. This proves the selected graph/toolchain can meet the static and reproducibility gates, not that the hash remains valid after implementation.

## Answer

Use the exact Rust 1.97.1 minimal toolchain in `rust-toolchain.toml`, with target `x86_64-unknown-linux-musl`, Cargo lockfile version 4, the direct dependency table above, and no floating dependency or tool version. `Cargo.toml` exact pins make deliberate upgrades visible; `Cargo.lock` pins every transitive source and checksum. The release build uses only the musl target and preserves `unsafe_code = "forbid"` in Volmap itself. Dependency-internal unsafe code is part of the dependency audit surface.

### Module and adapter shape

The inspection module owns byte access, validation, decryption, graph construction, revisions, diagnostics, coverage, resource policy, and normalized query operations behind one small synchronous interface. It accepts explicit file readers, cancellation, resource policy, and optional secret material rather than creating ambient dependencies. CLI human/JSON, Crossterm TUI, self-contained HTML, and Axum HTTP are adapters at the projection seam: they call the same query interface and shared serializers and never parse volume bytes. The HTML/web asset module embeds reviewed UTF-8 HTML/CSS/JavaScript with `include_bytes!`; there is no Node/npm build, remote font, CDN, runtime template, or filesystem asset lookup.

This seam is real because four adapters consume it. Internal parser/reader seams stay private and are exercised through the inspection interface or focused hostile-input tests. HTTP async work uses `spawn_blocking` around bounded synchronous queries rather than leaking Tokio types into the core. Terminal drawing is a deep module over Crossterm, not a public collection of widgets.

### Dependency sources and supply chain

For each release candidate:

1. Start from a reviewed commit with clean `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml`, web assets, provenance, license policy, and fixture hashes.
2. Build with `--locked`. Cargo may download missing registry sources into its normal cache; the lockfile's exact sources, versions, and checksums remain authoritative. Do not require or check in a `vendor/` source tree.
3. Audit every new/changed package, feature, source, build script, proc macro, `links` value, license, advisory, and unsafe/native-code surface. A crate that adds compiled C/C++, a shared library, TLS, an unapproved registry/git source, or a reciprocal/unknown license is release-blocking until replaced or explicitly company-approved and documented.
4. Generate deterministic third-party notices with `cargo-about` 0.9.2, policy/advisory/license results with `cargo-deny` 0.20.2, and CycloneDX JSON with `cargo-cyclonedx` 0.5.9 from a digest-pinned release-tools image. Record tool hashes and the exact Cargo lock hash. These tools are build/release inputs, not runtime dependencies.

The source archive contains the project source and `Cargo.lock`, not copied dependency sources; the installed product remains one executable. Generated notices include the Volmap Apache-2.0 license, CUBRID Apache-2.0 attribution with pinned source commit/profile, every third-party license text and package/version, and a statement that recovered artifacts are neither linked nor distributed. A single canonical notice blob is embedded in the executable and is reachable from `volmap licenses`, TUI help/about, web about/licenses, and every HTML export; `LICENSE`, `NOTICE`, SBOM, provenance, source checksum, and binary checksum also accompany the release bundle.

### Reproducible release gate

The release environment fixes toolchain/container digests, target, locale (`LC_ALL=C`), timezone (`TZ=UTC`), `SOURCE_DATE_EPOCH` to the source commit time, umask, features, and path remapping. Build twice from the same reviewed source revision in distinct absolute directories with:

```text
cargo build --release --locked --target x86_64-unknown-linux-musl
```

Both binaries must be byte-identical. `file` must identify a stripped x86-64 static PIE, `readelf -d` must have no `NEEDED`, `ldd` must report static/not-dynamic, and smoke tests must run the same snapshot through CLI human, JSON/JSONL, TUI model harness, HTML export, and web model endpoints and compare canonical facts, outcomes, diagnostics, coverage, and revision identifiers. Static linkage, duplicate-build equality, notices, SBOM, deny policy, advisory policy, non-disclosure tests, and all pinned fixtures are release blockers.

Built-in TLS remains excluded by the accepted remote-access contract. Axum exposes HTTP/1.1 only; loopback/SSH is the default and explicit internal `0.0.0.0` mode keeps mandatory bearer authentication and origin/host checks. Adding rustls/OpenSSL/native-tls or HTTP/2/WebSocket/multipart/compression features requires a new graph/security review rather than an incidental feature toggle.
