Type: grilling
Status: claimed (dhkim, 2026-08-26 session)
Blocked by:

# Name the volmap domain terms and the volatile-overlay ADR

## Question

Buffer state is volatile engine memory. It is not `Observed disk state` (CONTEXT.md defines that as bytes present in the volume files), it cannot be `Evidence` (no volume byte range, no validation rule), and a live inspector socket collides with `Standalone executable` ("no runtime dependency on … network services"). Decide, with the domain-modeling skill:

1. The new CONTEXT.md term(s) for the overlay — candidate: "Buffer residency overlay" or similar — with a definition that pins volatility, engine-memory provenance, per-observation timing, and non-membership in the inspection graph, plus an Avoid list (avoid: page status, sync status, live state).
2. The ADR granting a scoped exemption from ADR-0001's "every adapter projects the same committed facts" for this overlay (precedent: record-value parity was explicitly scoped out of Atlas parity once). The ADR must say which adapters may show the overlay and why the exemption does not leak into storage facts.
3. The amendment to `Standalone executable` (and README "Safety and scope") keeping the inspector connection strictly optional: absent server, absent socket, or refused handshake degrade to today's behavior with the overlay simply absent.

## Comments

## Answer
