# CBRD-27398 page-buffer inspector observations

This companion uses the matching CUBRID engine checkout's native page fixture
and socket controller. Build the engine on Linux with
`UNIT_TEST_PGBUF_INSPECTOR=ON` and install it. The native fixture is deliberately
not installed; select it from the matching CMake build directory.

Run on a dedicated CTP node or an isolated environment: the standard shell-suite
`init`/`finish` helpers manage CUBRID services. Load the installation's usual
`CUBRID`, `PATH` and `LD_LIBRARY_PATH` environment, then set:

```sh
export PGBUF_INSPECTOR_TEST_DIR=/path/to/engine/unit_tests/pgbuf_inspector
export PGBUF_INSPECTOR_FIXTURE=/path/to/build/bin/pgbuf_inspector_fixture
```

Copy the node's standard shell configuration to a run-specific file and set:

```ini
scenario=/path/to/testcases/shell/_06_issues/_26_2h/cbrd_27398
testcase_update_yn=false
testcase_retry_num=0
```

Remove any exclusion that covers this explicitly selected case. Run the standard
CTP entrypoint with the prepared configuration:

```sh
"$CTP_HOME/bin/ctp.sh" shell -c /path/to/run-specific-shell.conf
```

Require `Total Execution Case:1`, `Total Success Case:1`, `Total Fail Case:0`,
`Total Skip Case:0`, and the expected `cbrd_27398.sh` case identity. Missing
fixture/helpers are failures, never skips.

`controlled.log` records native clean/dirty page-state and partial-scan checks.
`attachment.log` independently records an unmodified `cub_server` default-off,
enabled socket, persistent identity and restart run. The controllers print their
retained raw evidence directories. `engine-identity.log` records source/testcase
HEADs, executable/library/helper hashes and fixture linkage. Retain these logs
with any uncommitted patch used for execution; a HEAD alone does not identify a
dirty worktree. Run in both Debug and Release environments.

Eviction/omission verification is pending the ticket 05 method decision; current
successful runs alone do not close the full ticket.
