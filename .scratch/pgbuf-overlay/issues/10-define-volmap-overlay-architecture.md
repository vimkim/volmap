Type: grilling
Status: open
Blocked by: 05, 06, 08

# Define the volmap overlay architecture

## Question

Given wire contract v1, the domain terms, and the security posture, fix the volmap-side architecture (consult codebase-design for the seam):

1. The overlay store — a sibling holder beside `Arc<LiveSource>` in `WebState` with its own `tokio::sync::watch` channel and poll task; strictly out of the inspection graph; never advancing snapshot generations or touching cursors.
2. Client — UDS connection lifecycle, reconnect/backoff, incarnation-change handling (server restart discards the overlay, never mixes observations across incarnations).
3. Publication — a separate `/api/v1` overlay resource vs an extra field on `WatchProjection`'s long-poll (SSE stays rejected); polling cadence; admission (its own semaphore like watchers).
4. Skew honesty — every overlay response carries its own observation time and capture token; the UI never implies the overlay and the disk facts are one instant.
5. Degradation — parameter off, socket absent, handshake refused, or version mismatch all degrade to the overlay being absent with a diagnostic-style explanation, not an error page.
6. Module seam — where the inspector client lives (a new `src/` module mirroring `follow.rs`'s role) and what its testable interface is (the in-memory adapter from the design reference).

## Comments

## Answer
