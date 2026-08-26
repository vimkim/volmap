# End-to-end commands for running Volmap with a current generated frontend.

set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Regenerate React, then serve demodb from a Rust debug build at http://127.0.0.1:7777.
serve-debug-demodb:
    just vite::frontend-generate-artifacts
    just cargo::serve-debug-demodb

# Regenerate React, then serve demodb from the optimized static-musl build at port 7777.
serve-release-demodb:
    just vite::frontend-generate-artifacts
    just cargo::serve-release-demodb
