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
# Starts its own fixture producer: no running CUBRID, parameter change, or restart needed.
# Click Enable observations in the browser. This preview does not attach to a real database.
[positional-arguments]
serve-overlay-preview host="192.168.4.2" port="7777":
    just vite::frontend-generate-artifacts
    python3 tools/serve-overlay-preview.py --listen "$1" --port "$2"

# Example: just user::check-overlay demodb /path/to/private/inspector.sock http://192.168.4.2:7777
# Socket may also be supplied through VOLMAP_RUNTIME_SOCKET. Uses cubrid from PATH.
# Checks the server's hidden enable_pgbuf_inspector parameter via paramdump -C --dump-flag 8.
# Requests one synthetic observation and only a greeting from CUBRID; changes no configuration.
# Exit 0: all checks pass; exit 1: unavailable, refused, unknown, or socket not supplied.
# Check synthetic preview, running database parameter, and real producer socket availability.
[positional-arguments]
check-overlay database="demodb" socket="" preview="http://192.168.4.2:7777":
    python3 tools/check-overlay.py --database "$1" --runtime-socket "$2" --preview-url "$3"

# Serve real demodb with page-buffer observations at http://192.168.4.2:7777.
# Select a page and click Enable observations. Socket can be overridden explicitly.
[positional-arguments]
serve-demodb-overlay socket="/tmp/pgbuf-inspector/ff9fa4791a9d59ece022093f466ef795.sock":
    just cargo::serve-debug --database demodb --databases-file /home/vimkim/.cub/db/CBRD-27398-pgbuf-inspector-contract/commondb/databases.txt --listen 192.168.4.2:7777 --runtime-page-buffer --runtime-socket "$1"
