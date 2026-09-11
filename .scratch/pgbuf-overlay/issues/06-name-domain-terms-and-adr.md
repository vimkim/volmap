Type: grilling
Status: resolved
Blocked by:

# Name the volmap domain terms and the volatile-overlay ADR

## Question

Buffer state is volatile engine memory. It is not `Observed disk state` (CONTEXT.md defines that as bytes present in the volume files), it cannot be `Evidence` (no volume byte range, no validation rule), and a live inspector socket collides with `Standalone executable` ("no runtime dependency on … network services"). Decide, with the domain-modeling skill:

1. The new CONTEXT.md term(s) for the overlay — candidate: "Buffer residency overlay" or similar — with a definition that pins volatility, engine-memory provenance, per-observation timing, and non-membership in the inspection graph, plus an Avoid list (avoid: page status, sync status, live state).
2. The ADR granting a scoped exemption from ADR-0001's "every adapter projects the same committed facts" for this overlay (precedent: record-value parity was explicitly scoped out of Atlas parity once). The ADR must say which adapters may show the overlay and why the exemption does not leak into storage facts.
3. The amendment to `Standalone executable` (and README "Safety and scope") keeping the inspector connection strictly optional: absent server, absent socket, or refused handshake degrade to today's behavior with the overlay simply absent.

## Comments

- Reclaimed from the stale `dhkim` claim on 2026-09-05 at the user's direction.
- The vocabulary and ADR had already landed in commit `d3a2eb4`. The user
  ratified those artifacts rather than introducing a second overlay term or a
  competing ADR.
- The user chose to add the missing optional-source degradation contract to
  README now. No runtime implementation is part of this decision.

## Answer

Use the existing domain model in `CONTEXT.md`:

- **Runtime page observation** is the generic, independently timed volatile
  reading about one physical page.
- **Runtime observation overlay** is the optional presentation layer over
  observed disk state. It does not modify inspection facts or revisions.
- **Page-buffer observation** is the CUBRID-specific runtime observation of
  semantic BCB evidence.
- **Runtime capability state**, **Observation freshness**, **Observation
  batch**, and **Observation coverage** describe availability and sampling
  without implying inspection validity or an atomic database-wide snapshot.
- **Runtime attachment** is the explicit, identity-proven association with one
  server incarnation.

Do not introduce `Buffer residency overlay`: it is narrower than the accepted
runtime-observation model and would duplicate its presentation concept.

Keep ADR-0006, **Treat runtime observations as loopback web capabilities**, as
the authoritative scoped exception. Runtime observations are web-only and
remain outside the inspection graph, revision chain, outcomes, diagnostics,
exports, and terminal-parity contract. Their absence, refusal,
incompatibility, or staleness changes only runtime capability state.

The `Standalone executable` definition remains unchanged: optional runtime
sources are not required runtime dependencies. README now states the matching
safety contract explicitly: an absent, refusing, or unsupported source leaves
ordinary disk inspection intact and simply provides no runtime overlay.
