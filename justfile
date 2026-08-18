# Local developer conveniences. Release acceptance remains defined by the
# resolved project specifications and the final ELF, not by these recipes.

set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

target := "x86_64-unknown-linux-musl"
artifact := "target/" + target + "/release/volmap"

default:
    @just --list

# Build the debug binary with the locked, offline dependency graph.
build:
    cargo build --locked --offline

# Build the optimized static musl artifact.
release:
    cargo build --release --locked --offline --target {{target}}

# Run every unit, integration, and documentation test.
test:
    cargo test --locked --offline

# Run the current Phase 0 zero-interface executable.
run:
    cargo run --locked --offline

# Install the static executable through Cargo's configured install root.
install:
    cargo install --path . --locked --offline --target {{target}}

# Format source files in place.
fmt:
    cargo fmt --all

# Fail if source files are not formatted.
fmt-check:
    cargo fmt --all -- --check

# Run Clippy for every target and feature with zero warnings allowed.
lint:
    cargo clippy --all-targets --all-features --locked --offline -- -D warnings

# Prove that the release artifact has no interpreter or shared-library needs.
elf-check: release
    file {{artifact}}
    ! readelf -l {{artifact}} | rg -q 'INTERP'
    ! readelf -d {{artifact}} | rg -q 'NEEDED'
    ! readelf -V {{artifact}} | rg -q 'GLIBC_'
    ldd {{artifact}} 2>&1 | rg -q 'statically linked|not a dynamic executable'

# Run all Phase 0 pre-commit gates.
verify: fmt-check test lint elf-check
    git diff --check

# Reserved until Wayfinder tickets 05, 06, 07, 08, 13, and 16 close.
serve:
    @echo "volmap serve is intentionally unavailable: the web architecture, contract, security, prototype, decoder scope, and release graph remain open." >&2
    @exit 2
