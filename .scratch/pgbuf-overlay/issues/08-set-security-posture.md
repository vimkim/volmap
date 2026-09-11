Type: grilling
Status: resolved
Blocked by: 03

# Set the security posture of an engine-connected serve

## Question

`volmap serve` is deliberately unauthenticated HTTP; connecting it to a live engine means whoever reaches the port sees engine-internal state. Decide:

1. The explicit opt-in — a `serve` flag enabling the overlay, never on by default.
2. Listener policy — whether overlay data is withheld on wildcard listeners (loopback-only overlay even when the mosaic listener is `0.0.0.0`), or allowed with a warning.
3. Trust on the inspector socket — `SO_PEERCRED` peer policy on the server side, handshake bound to database identity and server incarnation, per-database socket in a server-owned `0700` directory with `0600` socket mode (per the design reference), and what volmap verifies before trusting responses.
4. Authorization parity — `SHOW PAGE BUFFER STATUS` is DBA-only; decide the equivalent statement for who may attach the inspector (OS-user match via peer credentials, or a weaker stance for a debug-only feature).
5. What `/api/v1/session` and the web UI disclose about the overlay's presence and its data source.

## Comments

2026-08-26, premise from ticket 03: the channel is an AF_UNIX `SOCK_STREAM` socket with a JSON handshake carrying database identity and server incarnation — points 3 and 4 (peer trust, authorization parity) now attach to that concrete socket and handshake.

2026-09-04, premise from [Define the gating matrix](04-define-gating-matrix.md): `enable_pgbuf_inspector` is startup-only and a disabled server creates neither daemon nor socket. This ticket must include the enabled-but-unavailable policy (bind/path/permission/stale-socket failure: fail server startup or continue without the optional inspector) alongside socket ownership and stale-socket handling.

## Answer

Resolved with the user, 2026-09-05.

- **Explicit attachment:** page-buffer runtime attachment requires both an
  explicit runtime-observation enable option and an explicit inspector socket
  path. A path without enablement is a usage error; enablement without a path
  leaves the page-buffer capability `disabled` so other runtime sources can be
  enabled independently. There is no socket discovery, environment fallback,
  or implicit attachment merely because a socket exists.
- **HTTP boundary:** an attached `volmap serve` is loopback-only. Requesting
  runtime attachment with a wildcard or other non-loopback listener is a
  startup error, not a warning or silent downgrade. Ordinary serving without
  runtime attachment keeps its existing listener behavior. Remote use goes
  through SSH forwarding; version one adds neither HTTP authentication nor
  TLS. This adopts the already-normative boundary in
  [ADR-0006](../../../docs/adr/0006-runtime-observations-are-loopback-web-capabilities.md).
- **Authorization principal:** both processes authenticate their connected
  peer with Linux `SO_PEERCRED`. `cub_server` accepts only a client effective
  UID exactly equal to its own; Volmap likewise requires its effective UID,
  the private directory owner, socket owner, and connected server UID to
  agree. There is no root or group exception. An operator with root access can
  deliberately run Volmap as the server account. Unauthorized connections are
  closed before protocol exchange; Volmap presents the result as `refused`.
- **Socket namespace and permissions:** use CUBRID's existing Unix-socket root
  (`$CUBRID_TMP` when configured, otherwise
  `$CUBRID/var/CUBRID_SOCK`) with a dedicated server-owned
  `pgbuf-inspector/` directory at mode `0700`. The socket is mode `0600` and
  has a bounded opaque database key derived from the canonical database path
  plus database-creation identity, avoiding unsafe or overlong database names.
  The server reports the exact path locally; Volmap receives it only through
  explicit configuration.
- **Safe stale-socket recovery:** reject and preserve symlinks, non-sockets,
  wrong-owner entries, and any socket that accepts a connection. Reclaim only
  a same-owner socket confirmed stale by a failed connection attempt. Record
  the created socket's identity and unlink it at shutdown only if the pathname
  still names that exact socket. A path-length, directory, permission, bind,
  listen, or safe-reclamation failure leaves the inspector unavailable for
  the entire server incarnation; there is no background bind retry.
- **Optional failure policy:** if `enable_pgbuf_inspector=true` but safe socket
  activation fails, emit a prominent, stable startup error and continue
  starting `cub_server` without the inspector. Optional observation must not
  take down the database. Volmap sees socket absence as `unavailable`, distinct
  from `disabled`, `refused`, or `incompatible`.
- **Attachment identity:** after mutual peer verification, the handshake must
  match the complete persistent volume set using database-creation identity
  and each volume's `(volid, volume_creation, device, inode)`. Producer-only
  temporary volumes do not participate. Database name and path alone are
  insufficient; device and inode distinguish copied databases that retain the
  same on-disk creation fields. A separate unpredictable server-incarnation
  identifier scopes every observation and invalidates retained state on
  restart. Identity mismatch yields capability state `refused` with stable
  reason `identity-mismatch`.
- **Disclosure:** `/api/v1/session` and the UI expose the runtime capability
  state, peer/identity verification result, negotiated protocol version, a
  disclosure-safe database-identity fingerprint, abbreviated incarnation,
  observation times, and stable limitation/reason codes. They never expose
  the socket or volume path, UID/GID/PID, or raw OS errors. The source is named
  **CUBRID page-buffer observation**, never “live database”; runtime failure
  does not change inspection validity, outcome, diagnostics, or disk access.

The existing domain terms **runtime attachment**, **runtime capability state**,
and **page-buffer observation** already carry these meanings, so no new glossary
term or ADR is required by this resolution.
