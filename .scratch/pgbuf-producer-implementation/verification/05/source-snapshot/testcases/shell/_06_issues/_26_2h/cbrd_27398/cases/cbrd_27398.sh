#!/bin/bash
# CBRD-27398: native page-state preconditions plus the shipped inspector socket.
# Run on an isolated CTP node: the standard init/finish helpers control services.
# Supply helpers and the non-installed native fixture from the matching engine
# UNIT_TEST_PGBUF_INSPECTOR build. Missing prerequisites are failures, not skips.

. "$init_path/init.sh"
init test

if [ ! -x "${PGBUF_INSPECTOR_FIXTURE:-}" ] || \
   [ ! -f "${PGBUF_INSPECTOR_TEST_DIR:-}/controlled_observation.py" ] || \
   [ ! -f "${PGBUF_INSPECTOR_TEST_DIR:-}/server_attachment.py" ]; then
    write_nok 'missing matching inspector fixture/helpers'
    finish
    exit 1
fi

{
    printf 'CUBRID=%s\nfixture=%s\nhelpers=%s\n' "$CUBRID" "$PGBUF_INSPECTOR_FIXTURE" "$PGBUF_INSPECTOR_TEST_DIR"
    command -v cubrid
    cubrid_rel
    git -C "$PGBUF_INSPECTOR_TEST_DIR" rev-parse HEAD
    git rev-parse HEAD
    sha256sum "$CUBRID/bin/cub_server" "$CUBRID/lib/libcubrid.so" "$PGBUF_INSPECTOR_FIXTURE" \
        "$PGBUF_INSPECTOR_TEST_DIR/controlled_observation.py" "$PGBUF_INSPECTOR_TEST_DIR/server_attachment.py"
    ldd "$PGBUF_INSPECTOR_FIXTURE"
} > engine-identity.log 2>&1

if python3 "$PGBUF_INSPECTOR_TEST_DIR/controlled_observation.py" \
        --fixture "$PGBUF_INSPECTOR_FIXTURE" > controlled.log 2>&1; then
    write_ok
else
    cat controlled.log
    write_nok 'native page-state or synchronization verification failed'
fi

# This separately exercises an unmodified cub_server; fixture boot cannot stand
# in for default-off, normal attachment, complete/partial framing or restart.
if python3 "$PGBUF_INSPECTOR_TEST_DIR/server_attachment.py" --scan > attachment.log 2>&1; then
    write_ok
else
    cat attachment.log
    write_nok 'unmodified server attachment/lifecycle verification failed'
fi

finish
