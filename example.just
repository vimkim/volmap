# Throwaway examples for checking design assumptions against stable snapshots.

set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

toolchain := "1.97.1"
cargo := "cargo +" + toolchain

# Resolve one physical page to its allocating file and associated table name.
# Usage: just example page-table /copy/demodb_vinf 1 1000
page-table vinf volume page:
    {{cargo}} run --quiet --locked --example page_table_poc -- --vinf "{{vinf}}" "{{volume}}" "{{page}}"

# Interpret rows straight out of a database's volume files, by table name.
# Usage: just example interpret /path/to/db/demodb stadium olympic game
interpret db +tables:
    {{cargo}} run --quiet --locked --example interpret_probe -- "{{db}}" {{tables}}
