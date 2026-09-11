# Ticket 04 completed

Source commit: `e5f3cdf86908b8d23d4e09d28d22265bf240f7ad`.
Tested/reviewed tree: `7d37bcca5b7df5bb43805d3c0d49b05a3b277bb3`.

Debug and RelWithDebInfo each passed all 27 CTest entries, 61 inspector cases,
28 focused socket cases, and four separately enabled credential cases. Both
installed-server runs prove SQL update/commit/readback while two current server
send queues remain saturated, with excess admissions refused. Debug recorded
14 busy refusals and Release six. Restart, old-incarnation refusal, pathname
replacement preservation and failed-activation no-retry checks passed.

The two-axis review has no unresolved findings. Review corrections add scoped
client FD cleanup and actual kernel send-memory saturation proof before and
after SQL. The final hook reported no formatting changes.

Earlier compressed-fixture and socket-diagnostic precondition failures remain
in the manifest. Full-suite attempts with a long PL socket path and concurrent
shared OOS fixtures failed; final suites used a short CUBRID_TMP and ran
sequentially. No unrelated engine repair or accepted-limit change was made.
The worktree preset is restored to debug_gcc; the existing dirty CCI submodule
is not included in the commit.

See [manifest.json](manifest.json) for requirements, provenance, binary/artifact
hashes, actual counts, commands, environment, reviews and limitations. The
native no-wait argument uses audited trylock/unlock code plus the sampling and
real transport checks; it does not claim a controlled native held-mutex test.
External testcase delivery, consumer integration, develop port, known-page
state/eviction and performance evidence remain their separately owned gates.
