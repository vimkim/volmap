Type: grilling
Status: open
Blocked by: 05, 10

# Define the cross-repo verification strategy

## Question

How is the overlay proven correct on both sides without requiring a live engine in every test run?

1. A fake inspector endpoint speaking wire v1 (the design reference's in-memory adapter) as the primary volmap test double — scripted residency/dirty scenarios, version mismatches, refusals, incarnation changes.
2. Golden fixtures for overlay rendering (web; TUI per ticket 07's outcome) and for degradation states.
3. The real-engine gate — a debug-build `cub_server` + `demodb` integration check exercising the actual socket: where it runs (local release gate vs CI), and what minimal assertions it makes (residency of a just-fixed page, dirty after update, gone after eviction).
4. CUBRID-side tests — what the upstream PR carries (unit tests for the scan/serializer; whether a system-parameter-gated feature gets a shell test).
5. Wire-contract conformance — one shared description both repos test against, so contract drift is caught by tests, not integration.

## Comments

## Answer
