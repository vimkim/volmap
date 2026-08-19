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

# Run the command-line interface.
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

# Run all local pre-commit gates.
verify: fmt-check test lint elf-check
    cargo metadata --locked --offline --format-version 1 >/dev/null
    cmp LICENSE vendor/aes-0.9.2/LICENSE-APACHE
    ! rg -n 'path\+file:|download_url=file:|/home/|/tmp/' THIRD_PARTY_NOTICES.txt SBOM.cdx.json
    git diff --check

# Start the authenticated read-only HTTP viewer; pass normal serve arguments.
serve *args:
    cargo run --locked --offline -- serve {{args}}

# Regenerate deterministic notices and the CycloneDX SBOM with pinned tools.
release-artifacts:
    release/regenerate-artifacts.sh

# Run the clean-commit offline reproducibility and supply-chain audit.
release-audit:
    release/check.sh

# Run the deterministic full resource matrix; emits one JSON object per result.
resource-benchmark samples="30":
    VOLMAP_BENCH_SCALE=full VOLMAP_BENCH_SAMPLES={{samples}} cargo test --release --locked --offline --test resource_benchmark -- --ignored --nocapture
