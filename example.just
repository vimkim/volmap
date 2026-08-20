# Throwaway examples for checking design assumptions against stable snapshots.

set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

toolchain := "1.97.1"
cargo := "cargo +" + toolchain

# Resolve one physical page to its allocating file and associated table name.
# Usage: just example page-table /copy/demodb_vinf 1 1000
page-table vinf volume page:
    {{cargo}} run --quiet --locked --example page_table_poc -- --vinf "{{vinf}}" "{{volume}}" "{{page}}"
