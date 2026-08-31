Type: grilling
Status: open
Blocked by: 03

# Define the gating matrix

## Question

The permission grant is: CUBRID code interacting with volmap may change, debug mode only where it degrades performance, gated by a system parameter such as page-buffer monitoring. Turn that into a concrete matrix:

1. The system parameter — name (`pgbuf_monitoring`?), type (boolean vs graded level like `page_validation_level`), flags (`PRM_FOR_SERVER | PRM_HIDDEN` per the `pgbuf_monitor_locks` precedent, or client-visible via `PRM_FORCE_SERVER` so tooling can detect it), default off, cached-static-bool consumption idiom.
2. Which capabilities are release-safe behind the parameter — the state-only lock-free BCB scan already ships in release inside `SHOW PAGE BUFFER STATUS`, so state-only exposure arguably qualifies — versus debug-build-only (`NDEBUG` idiom): consistency digests, disk cross-reads, capture bracketing, any per-fix accounting.
3. Compile-time gate — a CMake-visible `option()` (the `ENABLE_SYSTEMTAP` model), a hand-define (the `ENABLE_CONTROLLER` model), or always-compiled-but-param-dead. This decides what a stock develop build can do.
4. What the server does when the parameter is off: inspector absent (socket never bound) versus bound-but-refusing.

## Comments

2026-08-26, premise from ticket 03: the transport is an AF_UNIX `SOCK_STREAM` socket (JSON-lines), owned by a dedicated inspector daemon; the SHOW variant is out of v1. Question 4's "socket never bound vs bound-but-refusing" now applies to that socket specifically.

## Answer
