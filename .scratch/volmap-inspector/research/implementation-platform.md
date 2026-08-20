# Implementation platform for Volmap Inspector

Date: 2026-08-18

## Decision

Implement Volmap Inspector in **Rust**, build releases for
`x86_64-unknown-linux-musl`, and treat the produced ELF—not the build command—as
the proof that the release is standalone.

Go is a fully viable alternative and is slightly easier for the packaging and
embedded-HTTP parts. The static-binary requirement alone does **not** favor Rust:
pure Go with `CGO_ENABLED=0` is the simpler recipe. Rust is the better choice for
this particular program because its core is a long-lived, corruption-tolerant
decoder for source-traced C/C++ disk structures. Rust offers stronger tools for
making byte ranges, decoded identifiers, page kinds, validated lengths, and
partially decoded values distinct in the type system, while retaining explicit
allocation and lifetime control. That advantage applies to the majority of the
risk in this tool; HTTP and terminal presentation are adapters around the same
inspection core.

The choice is conditional on keeping the parser in safe Rust and keeping native
dependencies out of the release graph. If the implementation team will not
maintain idiomatic Rust, Go is a sound fallback; a careless Rust decoder full of
`unsafe`, layout casts, and unchecked arithmetic would lose the reason for this
decision.

## Comparison

| Concern | Rust | Go | Finding |
|---|---|---|---|
| Static Linux x86-64 | The musl target statically links its C runtime by default. | A pure-Go build with cgo disabled links without glibc. | Both satisfy the requirement; Go has the simpler toolchain. |
| Corrupt binary input | Safe slices, checked conversions/arithmetic, `Result`, enums/newtypes, and fallible reservation support a parser whose invariants are explicit. | Slices and fixed-width integers are safe and suitable, but out-of-range indexing panics and domain invariants rely more on conventions and explicit checks. | Rust has the meaningful advantage. |
| Layout and endianness | `from_le_bytes`/`from_be_bytes` make byte order explicit; Rust's default struct layout is not stable and must not be used as an on-disk representation. | `encoding/binary` directly supports explicit byte order and fixed-size values. | Equivalent if both parse fields explicitly; neither should cast C structs over bytes. |
| Bounded I/O and memory | `FileExt::read_exact_at`, borrowed page slices, checked arithmetic, and `Vec::try_reserve` support page-at-a-time scans and fallible large allocations without a GC. | `io.ReaderAt`/`SectionReader` support the same page-at-a-time design; the runtime also has a soft memory limit. | Both can be bounded, but Rust makes per-allocation and ownership discipline easier to enforce. |
| TUI | Ratatui provides an immediate-mode TUI with Crossterm as its default backend. | Bubble Tea provides an Elm-style model/update/view TUI. | Both are capable; this does not decide the language. |
| Embedded web | Axum/Tokio plus compile-time embedded assets work well but add third-party packages. | `net/http` and `embed` are standard-library facilities. | Go has the clear dependency-simplicity advantage. |
| Dependency and license surface | Rust itself is MIT/Apache-2.0; the likely web/TUI stack is permissive but produces a larger transitive package graph that must be audited. | Go is BSD-style; the HTTP server and asset embedding require no third-party module, while Bubble Tea is MIT. | Go is easier to audit. Rust remains acceptable only with a locked, reviewed dependency set and generated notices/SBOM. |
| Reproducibility | `Cargo.lock`, `--locked`, a pinned toolchain, fixed build environment, and path remapping are required. Build-time downloads are allowed. | `go.mod`/`go.sum`, a pinned toolchain, `-trimpath`, and controlled VCS/build IDs give a shorter recipe. | Go is easier; both need a release pipeline and byte-for-byte checks. |
| Executable size | Static musl plus LTO/strip/panic-abort was small in the local representative check. | The Go runtime and `net/http` made the representative binary larger even after stripping. | Rust led this check, but final size must be measured again with the actual TUI and decoder. |
| Decoder maintainability | Algebraic enums, exhaustive matching, newtypes, borrowing, and compiler-checked ownership fit a growing family of page-specific decoders. | Simpler syntax, fast compilation, a strong compatibility promise, and fewer framework dependencies lower onboarding cost. | Rust is preferred for correctness-oriented evolution; Go wins if team fluency dominates. |

## Primary-source evidence

### Static linking

Rust's linkage reference lists `x86_64-unknown-linux-musl` among targets whose C
runtime is static by default and explicitly recommends inspecting the output to
confirm linkage ([Rust Reference: static and dynamic C runtimes](https://doc.rust-lang.org/stable/reference/linkage.html#static-and-dynamic-c-runtimes)).
A Rust executable also links Rust dependencies into a single distributable
binary unless dynamic linkage is requested
([Rust Reference: linkage](https://doc.rust-lang.org/stable/reference/linkage.html#linkage)).

Go's cgo documentation explains that packages such as `net` may select cgo and
participate in external/dynamic linking; consequently the release must disable
cgo, not merely assume that Go binaries are static
([Go cgo source documentation](https://go.dev/src/cmd/cgo/doc.go)). The Go build
environment defines `CGO_ENABLED` as the switch controlling cgo support
([Go command documentation](https://pkg.go.dev/cmd/go#hdr-Environment_variables)).

For either language, a package that introduces native code can invalidate the
simple recipe. The release gate therefore checks the final ELF for both a program
interpreter and `DT_NEEDED` entries.

### Binary parsing and bounded work

Rust's ownership rules are compiler checked and do not add a garbage collector
or ownership runtime
([The Rust Book: ownership](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html)).
Safe slice access can return `Option`, while unchecked access is explicitly
unsafe and out-of-bounds use is undefined behavior
([Rust slice documentation](https://doc.rust-lang.org/std/primitive.slice.html)).
Fixed-width integers provide explicit byte-order constructors such as
`u32::from_le_bytes`
([Rust `u32`](https://doc.rust-lang.org/std/primitive.u32.html#method.from_le_bytes)).
Unix `FileExt::read_exact_at` fills a caller-sized buffer from an explicit offset
([Rust `File`](https://doc.rust-lang.org/std/fs/struct.File.html#method.read_exact_at)),
and `Vec::try_reserve` reports capacity overflow or allocation failure instead of
requiring an infallible reserve
([Rust `Vec`](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.try_reserve)).

Rust's own layout reference warns that the default Rust representation gives no
general field-layout guarantee; even C representation involves padding and
alignment rules
([Rust Reference: type layout](https://doc.rust-lang.org/reference/type-layout.html)).
Therefore a CUBRID C/C++ struct definition is evidence for offsets and sizes, not
permission to transmute a page into a Rust struct.

Go's standard `encoding/binary` package provides explicit `ByteOrder` decoding
of fixed-size values
([Go `encoding/binary`](https://pkg.go.dev/encoding/binary)). `io.ReaderAt`
requires reads at a specified offset, while `SectionReader` constrains a reader
to a byte interval
([Go `io`](https://pkg.go.dev/io#ReaderAt)). Go checks slice/index bounds, but the
language specification says an out-of-range runtime index causes a panic
([Go specification: index expressions](https://go.dev/ref/spec#Index_expressions)).
That is memory-safe, but it is not the desired malformed-page error path. A Go
implementation would need the same explicit precondition checks and panic-free
parser API. Go's `SetMemoryLimit` is a useful process-wide soft limit, but its
documentation excludes some runtime-external memory
([Go `runtime/debug`](https://pkg.go.dev/runtime/debug#SetMemoryLimit)).

Neither language automatically prevents integer-overflow mistakes, pathological
counts, allocation amplification, or CPU denial of service. The architecture
must require checked offset/length arithmetic, page-sized reads, hard limits on
all disk-derived counts, no unbounded recursive chain traversal, and structured
errors rather than panics.

### CLI, TUI, and web support

Ratatui describes itself as a lightweight TUI library, uses Crossterm by default,
and supports immediate-mode rendering
([Ratatui documentation](https://docs.rs/ratatui/latest/ratatui/)); its project is
MIT licensed ([Ratatui repository](https://github.com/ratatui/ratatui)). Bubble
Tea describes an Elm-style framework for inline and full-window terminal
applications and is MIT licensed
([Bubble Tea repository](https://github.com/charmbracelet/bubbletea)). These are
different programming models, not a capability gap.

Go can embed an asset tree into a read-only `embed.FS`, which interoperates
directly with `net/http`
([Go `embed`](https://pkg.go.dev/embed)); the standard `net/http` package provides
the server and exposes timeouts and maximum header sizing on `http.Server`
([Go `net/http`](https://pkg.go.dev/net/http#Server)). Rust needs ecosystem
components; Axum provides routing/extractors on the Tower/Hyper ecosystem and its
own crate forbids unsafe code
([Axum repository](https://github.com/tokio-rs/axum)). This is a genuine Go
advantage, but presentation is not the risky core of Volmap Inspector.

### Reproducibility and licensing

Cargo documents that `Cargo.lock` records the versions used by a successful
build, and `--locked` prevents resolution from changing them
([Cargo FAQ](https://doc.rust-lang.org/cargo/faq.html#why-have-cargolock-in-version-control),
[Cargo command options](https://doc.rust-lang.org/cargo/commands/cargo.html#manifest-options)).
Builds may fetch the sources named by the lockfile from their registries; an
offline dependency mirror is not part of the product requirement. Rustc
supports source-path remapping for output normalization
([rustc path remapping](https://doc.rust-lang.org/rustc/remap-source-paths.html)).

Go modules record dependency versions and checksums in `go.mod` and `go.sum`
([Go: managing dependencies](https://go.dev/doc/modules/managing-dependencies)).
The Go build command's `-trimpath` removes filesystem paths, while
`-buildvcs=false` omits VCS stamping
([Go command](https://pkg.go.dev/cmd/go#hdr-Compile_packages_and_dependencies));
the linker documents `-buildid`, `-s`, and `-w`
([Go linker](https://pkg.go.dev/cmd/link)).

Official Rust projects are generally dual MIT/Apache-2.0
([Rust licensing policy](https://rust-lang.org/policies/licenses/)); Go is under a
BSD-style license ([Go project](https://go.dev/project)). These language licenses
do not cover ecosystem dependencies, so the final lockfile—not this comparison—
is the authoritative license inventory.

Both languages make long-term source stability commitments: Rust uses opt-in
editions so stable features and crates from different editions continue to
interoperate ([Rust Edition Guide](https://doc.rust-lang.org/edition-guide/editions/));
Go's compatibility promise intends Go 1 programs to keep compiling and running
across later Go 1 releases, subject to documented exceptions
([Go 1 compatibility](https://go.dev/doc/go1compat)).

## Reproducible local check

This check was deliberately small: each program decoded a little-endian `u32`
and exposed embedded HTML plus a JSON endpoint. The Rust sample used
Axum/Tokio/Serde; the Go sample used only `net/http`, `embed`, `encoding/binary`,
and `encoding/json`. It measured packaging characteristics, not parser throughput
or the expected final product size.

Host tools:

```text
rustc 1.97.1 (8bab26f4f 2026-07-14)
cargo 1.97.1 (c980f4866 2026-06-30)
go version go1.26.5 linux/amd64
```

Build forms:

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --locked --target x86_64-unknown-linux-musl

CGO_ENABLED=0 GOOS=linux GOARCH=amd64 \
  go build -trimpath -buildvcs=false \
  -ldflags='-s -w -buildid=' -o volmap-go .
```

The Rust release profile used `strip = "symbols"`, `lto = true`,
`codegen-units = 1`, and `panic = "abort"`. Results:

| Check | Rust | Go |
|---|---:|---:|
| File size | 942,752 bytes | 6,066,302 bytes |
| `file` | static PIE, stripped | statically linked, stripped |
| ELF interpreter | none | none |
| `DT_NEEDED` | none | none |
| Two clean-build SHA-256 values | identical | identical |
| Third-party dependency units in this sample | 45 Cargo packages (46 including the application) | 0 Go modules |

The size result is not an apples-to-apples framework benchmark: Rust used a
third-party asynchronous stack and aggressive LTO, while Go included its runtime
and standard HTTP server. It nevertheless disproves the assumption that choosing
Go necessarily yields the smaller finished executable.

The current stable Cargo rejected the profile-level `trim-paths` setting as
unstable. The two same-path clean Rust builds were byte-identical without it, but
that is evidence from one environment, not a cross-host guarantee. The release
pipeline should therefore use a fixed source path inside a pinned container and,
where necessary, stable rustc `--remap-path-prefix`; it must compare clean-build
hashes in CI before claiming reproducibility.

## Required Rust release strategy

1. Pin an exact stable Rust toolchain and the `x86_64-unknown-linux-musl` target in
   `rust-toolchain.toml`; build in a pinned Linux container with a fixed source
   path.
2. Commit `Cargo.lock`. Release with `cargo build --release --locked --target
   x86_64-unknown-linux-musl`. Build-time network access is allowed; the final
   ELF and runtime distribution tests, not dependency-source location, prove
   binary mobility.
3. Use a release profile with LTO, one codegen unit, symbol stripping, and
   `panic = "abort"`. Keep overflow checks enabled in release for the decoder or
   use checked arithmetic everywhere; performance tests may decide the exact
   profile only after correctness is proven.
4. Prefer pure-Rust dependencies and disable unnecessary default features. Do not
   depend on CUBRID libraries, glibc, OpenSSL/native TLS, ncurses, or other shared
   libraries. The planned HTTP viewer does not need in-process TLS because remote
   use is protected separately by its token and an SSH/VPN/reverse-proxy boundary.
5. Gate each release with all of the following:

   ```sh
   file target/x86_64-unknown-linux-musl/release/volmap
   readelf -l target/x86_64-unknown-linux-musl/release/volmap
   readelf -d target/x86_64-unknown-linux-musl/release/volmap
   ldd target/x86_64-unknown-linux-musl/release/volmap
   ```

   Acceptance requires a static/static-PIE ELF, no `INTERP` program header, no
   `NEEDED` dynamic entries, and `ldd` reporting a non-dynamic/static executable.
   Also run the binary in a minimal glibc-free Linux container.
6. Generate and review a dependency/license inventory from the exact lockfile for
   each release. Pin features as well as versions and reject native dynamic
   dependencies.

## Architecture constraints created by this decision

- Put disk parsing in a presentation-independent crate and apply
  `#![forbid(unsafe_code)]` there. Any unavoidable unsafe code must be outside the
  decoder crate and separately justified and tested.
- Read fixed-size headers/pages with positional I/O. Do not read a whole database
  into memory and do not make `mmap` the default; page-at-a-time reads fit the
  strict offline snapshot and change-detection policy better.
- Decode every scalar from bytes with explicit endianness. Do not use `repr(C)`,
  `repr(packed)`, pointer casts, transmute, or bindgen-generated layout as the
  parser.
- Represent physical identifiers and offsets with fixed-width/newtype values;
  convert to `usize` only after bounds checks. Use checked add/multiply for every
  disk-derived offset, length, count, slot, and OOS-chain step.
- Bound memory and work at API boundaries: maximum page size, slots per page,
  chain length, decoded records, cache bytes, request concurrency, JSON response
  size, and raw-hex bytes. Malformed input returns diagnostics; it must not panic.
- Let CLI, TUI, HTML export, and localhost/remote HTTP service consume the same
  immutable typed inspection model. Serialize explicit versioned DTOs rather than
  exposing parser structs as the JSON contract.
- Treat Ratatui/Crossterm and Axum/Tokio as reasonable initial candidates, not as
  part of this platform decision. Later architecture/prototype tickets should
  select their exact versions/features and remeasure binary size and dependency
  footprint.

## Go fallback

If maintainers decide that sustained Rust ownership is unavailable, choose Go
without weakening the product requirements. The fallback release recipe is
`CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -trimpath -buildvcs=false
-ldflags='-s -w -buildid='`, followed by the same ELF gates and glibc-free runtime
test. The decoder must avoid `unsafe`, recover no parser panics as normal control
flow, use `ReaderAt`/`SectionReader`, validate before every slice, cap all
allocations and traversals, and keep the JSON/TUI/web layers out of the page
decoder package.
