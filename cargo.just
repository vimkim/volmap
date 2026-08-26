# Rust build and serve primitives. They embed the currently generated frontend assets.

set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

toolchain := "1.97.1"
cargo := "cargo +" + toolchain
target := "x86_64-unknown-linux-musl"

# Start the Rust debug server with explicit serve arguments; does not regenerate React.
serve-debug *args:
    {{cargo}} run --locked -- serve {{args}}

# Serve demodb from a Rust debug build at http://127.0.0.1:7777; does not regenerate React.
serve-debug-demodb:
    {{cargo}} run --locked --target {{target}} -- serve --database demodb --listen 0.0.0.0:7777

# Serve demodb from the optimized static-musl build at port 7777; does not regenerate React.
serve-release-demodb:
    {{cargo}} run --release --locked --target {{target}} -- serve --database demodb --listen 0.0.0.0:7777
