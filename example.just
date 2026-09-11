# Developer examples for checking production projections against stable snapshots.

set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

toolchain := "1.97.1"
cargo := "cargo +" + toolchain

# Resolve one physical page to its allocating file and associated table name.
# Usage: just example page-table /copy/demodb_vinf 1 1000
page-table vinf volume page:
    {{cargo}} run --quiet --locked --example page_file_association -- --vinf "{{vinf}}" "{{volume}}" "{{page}}"

# Interpret rows straight out of a database's volume files, by table name.
# Usage: just example interpret /path/to/db/demodb stadium olympic game
interpret db +tables:
    {{cargo}} run --quiet --locked --example interpret_probe -- "{{db}}" {{tables}}

# Query one VPID through CUBRID's inspector socket (Python stdlib, Linux).
# Usage: just example pgbuf 0 0 /tmp/pgbuf-inspector/<identity>.sock
pgbuf volid pageid socket:
    python3 examples/pgbuf_socket.py --socket "{{socket}}" "{{volid}}" "{{pageid}}"

# Current contract-worktree demodb endpoint; override socket after recreation.
# Requires demodb already started with enable_pgbuf_inspector=yes.
pgbuf-demodb volid="0" pageid="0" socket="/tmp/pgbuf-inspector/ff9fa4791a9d59ece022093f466ef795.sock":
    python3 examples/pgbuf_socket.py --socket "{{socket}}" "{{volid}}" "{{pageid}}"
