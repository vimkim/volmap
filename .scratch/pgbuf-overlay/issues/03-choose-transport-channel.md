Type: grilling
Status: resolved
Blocked by: 02

# Choose the inspector transport channel

## Question

How does volmap ask a running `cub_server` for buffer state? Candidates, from the survey's "cheapest viable exposure paths":

1. AF_UNIX socket following the in-tree `controller.hpp` / `ENABLE_CONTROLLER` pattern — the design reference's recommendation (`docs/live-page-buffer-inspection.md`); volmap talks directly, no CUBRID client protocol.
2. Per-BCB `SHOW` statement — zero new IPC, DBA-gated, but volmap's standalone contract forbids CUBRID client libraries, so volmap would have to shell out to csql or embed a protocol.
3. `NET_SERVER_PGBUF_*` request + `cubrid` subcommand (memmon clone) — compact binary per poll, but same client-protocol problem; volmap would shell out to an installed utility.
4. Shared memory — cheapest per poll, no precedent in cub_server, most new code, exposes layout.

Constraints to weigh: volmap's `Standalone executable` term (no CUBRID libraries, no network services as a hard dependency); tokio already includes UDS so JSON-over-UDS needs no new volmap crate, while any framed binary format triggers the pinned-dependency SBOM/notices/license release gates; per-poll server cost and poll cadence; DBA-only authorization equivalents (`SHOW` is `only_for_dba`; a socket needs its own peer policy); AF_UNIX `SOCK_DGRAM` datagram sizing versus a pool of ~100k BCBs (streaming or chunking may force `SOCK_SEQPACKET`/`SOCK_STREAM` instead). Decide the channel, the socket type and framing direction, and whether a fallback channel (e.g. the SHOW variant as a debugging aid) is also in scope.

## Comments

## Answer

Resolved with the user, 2026-08-26.

- **Primary channel: AF_UNIX socket.** Volmap connects directly to a
  server-owned local socket — no CUBRID client protocol, no installed-utility
  subprocess, no runtime CUBRID dependency in volmap. Matches the design
  reference's recommended seam and the in-tree `controller.hpp` /
  `ENABLE_CONTROLLER` precedent for an AF_UNIX control channel.
- **Socket type and encoding: `SOCK_STREAM` with a versioned JSON-lines
  protocol** — a JSON handshake first (protocol version negotiation, database
  identity, server incarnation), then newline-delimited JSON records.
  Rationale: resident-set responses are pool-sized (100k+ records), past
  datagram comfort; JSON adds **zero new dependencies on either side**
  (volmap already ships `serde_json`; CUBRID already builds rapidjson 1.1.0 —
  `3rdparty/CMakeLists.txt:446-455` — and flat records can be
  `snprintf`-emitted); streaming frames give natural backpressure; the
  channel is debuggable with `socat`. The `controller.hpp` shape itself
  (`SOCK_DGRAM` + POD structs) is explicitly NOT reused: datagram sizing and
  the never-serialize-engine-structs rule both forbid it.
- **Fallback channel: the per-BCB `SHOW` statement variant is deferred out of
  v1** (recorded in the map's Out of scope). The JIRA issue may cite it as
  future work; it also stands as the ready-made fallback position if upstream
  review resists the socket.
- Handoff note (technical detail, not a map decision): the socket fd should
  be owned by a dedicated inspector `cubthread::daemon`
  (`pgbuf_daemons_init` pattern), bound only when the parameter is on — the
  coordinator's epoll loop is connection-pool-specific and compiled out.
