Type: research
Status: open
Blocked by: 01, 05, 06, 07, 08, 09, 13, 14, 15

# Evaluate the final Rust dependency and reproducible-release graph

## Question

Once the parser architecture and interface prototypes have selected their actual capabilities, what exact pinned Rust toolchain, crates, features, native-code exclusions, embedded assets, vendoring strategy, and release process should Volmap Inspector use? Produce a lockfile-level recommendation that proves `x86_64-unknown-linux-musl` static linkage, offline/reproducible construction, minimal attack and license surface, required Apache-2.0/CUBRID and third-party notice delivery, an SBOM, and identical behavior across CLI, TUI, HTML export, and web-service modes. Identify any dependency that introduces C/C++, shared libraries, build scripts, TLS, reciprocal licensing, or unreproducible inputs, and recommend an evidence-backed replacement or explicit approval gate.

## Comments
