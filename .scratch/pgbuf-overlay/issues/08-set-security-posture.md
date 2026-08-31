Type: grilling
Status: open
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

## Answer
