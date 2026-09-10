# End-to-end commands for running Volmap with a current generated frontend.

set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

toolchain := "1.97.1"
cargo := "cargo +" + toolchain

# Regenerate React, then serve demodb from a Rust debug build at http://127.0.0.1:7777.
serve-debug-demodb:
    just vite::frontend-generate-artifacts
    just cargo::serve-debug-demodb

# Regenerate React, then serve demodb from the optimized static-musl build at port 7777.
serve-release-demodb:
    just vite::frontend-generate-artifacts
    just cargo::serve-release-demodb

# Print the focused Page renderer at all supported terminal tiers.
# This non-interactive preview complements the production `tui` command.
tui-page-preview:
    {{cargo}} test --quiet --locked --lib tui::focused::tests::print_page_mode_preview -- --ignored --nocapture --test-threads=1

# Print selected-record interpretation at all supported terminal tiers.
# This non-interactive preview complements the production `tui` command.
tui-interpretation-preview:
    {{cargo}} test --quiet --locked --lib tui::focused::tests::print_interpretation_mode_preview -- --ignored --nocapture --test-threads=1

# Synthetic dense overlay on the requested LAN address; Ctrl-C cleans up its fixture server.
[positional-arguments]
serve-overlay-preview host="192.168.4.2" port="7777":
    just vite::frontend-generate-artifacts
    python3 tools/serve-overlay-preview.py --listen "$1" --port "$2"
