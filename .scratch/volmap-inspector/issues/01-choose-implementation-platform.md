Type: research
Status: resolved
Blocked by:

# Choose the implementation platform for the standalone inspector

## Question

Should Volmap Inspector be implemented in Rust or Go, given the non-negotiable requirement for one Linux x86-64 executable with no runtime glibc or CUBRID-library dependency? Compare, using primary sources and small reproducible checks where useful: truly static linking and verification; binary-format parsing safety; control over on-disk layouts and endianness; bounded memory and streaming I/O; TUI and embedded-web support; compile-time and runtime dependency footprint; reproducible builds; executable size; licensing; and maintainability for source-traced CUBRID page decoders. Recommend one platform and identify the exact static toolchain strategy and any caveats that later architecture tickets must honor.

## Comments

## Answer

Choose **Rust** with the `x86_64-unknown-linux-musl` target. Go is the easier
pure-static and embedded-HTTP option, but Rust's stronger invariant modeling,
safe borrowing, explicit allocation control, and page-decoder type structure are
the better fit for a corruption-tolerant, source-traced CUBRID inspector. The
static requirement by itself does not decide the language; both platforms passed
representative local no-`INTERP`/no-`DT_NEEDED` checks.

Release builds must pin the Rust toolchain, musl target, and `Cargo.lock`, build
with `--locked` in a fixed environment, exclude native/shared-library dependencies,
and verify the final ELF plus execution in a glibc-free container. Build-time
network access is allowed; it is not part of the binary-mobility requirement. The parsing
crate must forbid unsafe code, decode explicit byte offsets and endianness, use
checked arithmetic and bounded positional reads, and never cast C/C++ layouts over
disk bytes. CLI, TUI, and web interfaces remain adapters over the same immutable
typed inspection model.

Full evidence, measured caveats, the exact release strategy, and the Go fallback
are in [Implementation platform for Volmap Inspector](../research/implementation-platform.md).
