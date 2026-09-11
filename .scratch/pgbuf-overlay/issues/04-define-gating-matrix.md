Type: grilling
Status: resolved
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

Resolved with the user, 2026-09-04.

- **System parameter:** `enable_pgbuf_inspector`, a Boolean with
  `PRM_FOR_SERVER | PRM_HIDDEN`, default `false`. It is startup-only: do not
  add `PRM_USER_CHANGE`, `PRM_RELOADABLE`, `PRM_FOR_CLIENT`, or
  `PRM_FORCE_SERVER`. Read it once while initializing the inspector; do not
  query the parameter from scan or page-buffer hot paths. The name describes
  explicit access to a pull-based inspector rather than implying broad or
  continuous page-buffer monitoring.
- **Disabled behavior:** when the parameter is false, create neither the
  inspector daemon nor its socket. Changing the parameter requires a server
  restart. Volmap discovers the optional capability through the Unix-socket
  attachment attempt, not through CUBRID parameter synchronization.
- **Build gate:** add no inspector-specific CMake option or hand-defined macro
  for v1. Compile the state-only producer into supported Unix `SERVER_MODE`
  builds and leave it inert by default behind the runtime parameter. Windows
  and non-server binaries expose no endpoint.
- **Release-safe capability:** after explicit enablement, Release,
  RelWithDebInfo, Debug, and OptDebug builds may expose the bounded v1
  resident-set scan: semantic scalar state, one coherent atomic latch-word
  load per BCB, scan framing, and the already-decided concurrency/rate/
  backpressure bounds. This remains best-effort observation, not an atomic
  pool snapshot or consistency proof.
- **Debug-only future capability:** protected page-image capture and copying,
  memory/main-volume/DWB digests, direct disk/DWB/TDE work, optimistic A/B
  capture bracketing, consistency classification, and per-fix or transition
  accounting are unavailable in `NDEBUG` builds. When that capability is
  designed, give it a separate CMake-visible gate and advertise it separately
  in protocol capability negotiation; do not turn the v1 Boolean into a
  graded monitoring level prematurely.
- **Wire consequence:** a disabled server normally cannot emit
  `parameter-off`, because no listener exists. Socket absence represents an
  unavailable/disabled runtime attachment. A client may accept
  `parameter-off` defensively for forward compatibility, but the v1 producer
  does not emit it.
