# Local developer conveniences. Release acceptance remains defined by the
# resolved project specifications and the final ELF, not by these recipes.

set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

toolchain := "1.97.1"
cargo := "cargo +" + toolchain
target := "x86_64-unknown-linux-musl"
artifact := "target/" + target + "/release/volmap"
install_root := env_var_or_default("VOLMAP_INSTALL_ROOT", env_var("HOME") + "/.local")

# Throwaway, manually driven examples live in their own command namespace.
mod example 'example.just'
# End-to-end commands for people running the application.
mod user 'user.just'
# Frontend build and verification primitives.
mod vite 'vite.just'
# Rust build and serve primitives that use the currently generated frontend.
mod cargo 'cargo.just'

default:
    @just --list

# Build the debug binary with the locked dependency graph.
build-debug:
    {{cargo}} build --locked

# Build the optimized static musl artifact.
build-release:
    {{cargo}} build --release --locked --target {{target}}

# Run every unit, integration, and documentation test in the debug test profile.
test-debug:
    {{cargo}} test --locked

# Run the command-line interface in the debug profile.
run-debug:
    {{cargo}} run --locked

# Build and install the release musl executable to ~/.local/bin by default.
install-release:
    {{cargo}} install --path . --locked --target {{target}} --root "{{install_root}}"

# Rebuild and replace the release musl executable in the local install root.
reinstall-release:
    {{cargo}} install --path . --locked --target {{target}} --root "{{install_root}}" --force

# Remove the executable and Cargo's corresponding install metadata.
uninstall:
    {{cargo}} uninstall --root "{{install_root}}" volmap

# Format source files in place.
fmt:
    {{cargo}} fmt --all

# Fail if source files are not formatted.
fmt-check:
    {{cargo}} fmt --all -- --check

# Run Clippy for every target and feature with zero warnings allowed.
lint:
    {{cargo}} clippy --all-targets --all-features --locked -- -D warnings

# Prove that the release artifact has no interpreter or shared-library needs.
elf-check-release: build-release
    file {{artifact}}
    ! readelf -l {{artifact}} | rg -q 'INTERP'
    ! readelf -d {{artifact}} | rg -q 'NEEDED'
    ! readelf -V {{artifact}} | rg -q 'GLIBC_'
    ldd {{artifact}} 2>&1 | rg -q 'statically linked|not a dynamic executable'

# Run all local pre-commit gates.
verify: fmt-check test-debug lint elf-check-release frontend-check
    {{cargo}} metadata --locked --format-version 1 >/dev/null
    ! rg -n 'path\+file:|download_url=file:|/home/|/tmp/' THIRD_PARTY_NOTICES.txt SBOM.cdx.json
    git diff --check

# Compatibility alias for the low-level Rust debug server.
serve-debug *args:
    just cargo::serve-debug {{args}}

# Compatibility alias; regenerates the frontend before serving demodb in debug.
serve-debug-demodb:
    just user::serve-debug-demodb

# Compatibility alias; regenerates the frontend before serving demodb in release mode.
serve-release-demodb:
    just user::serve-release-demodb

# Start the optimized static musl TUI for the local demodb database.
tui-release-demodb:
    {{cargo}} run --release --locked --target {{target}} -- tui --database demodb

# Regenerate deterministic notices and the CycloneDX SBOM with pinned tools.
release-artifacts:
    release/regenerate-artifacts.sh

# Compatibility alias for the frontend dependency installation primitive.
frontend-install:
    just vite::frontend-install

# Compatibility alias for installing the pinned browser revisions.
frontend-install-browsers:
    just vite::frontend-install-browsers

# Compatibility alias for regenerating the embedded React bundle.
frontend-artifacts:
    just vite::frontend-generate-artifacts

# Compatibility alias for the complete frontend verification primitive.
frontend-check:
    just vite::frontend-check

# Run the clean-commit reproducibility and supply-chain audit.
release-audit:
    release/check.sh

# Run the deterministic full resource matrix; emits one JSON object per result.
resource-benchmark-release samples="30":
    VOLMAP_BENCH_SCALE=full VOLMAP_BENCH_SAMPLES={{samples}} {{cargo}} test --release --locked --test resource_benchmark -- --ignored --nocapture
