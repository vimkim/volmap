Type: grilling
Status: resolved
Blocked by: 06

# Decide the TUI parity treatment for the overlay

## Question

`Terminal interaction parity` requires the TUI to preserve the web viewer's semantic visual distinctions, the parity verification contract defines parity facts as exact-revision Projection facts, and the semantic-rendering decision fixed a closed, non-mergeable seven-channel page strip. A volatile overlay violates all three premises as written. Decide:

1. Whether the overlay becomes an eighth strip channel (with terminal presentation-profile fallbacks) or a documented scoped exemption from parity (precedent exists for record-value parity).
2. What parity even means for a fast-changing overlay in a terminal — same facts at same cadence, same facts on demand, or web-only until proven needed.
3. Where the decision is recorded: the parity contract tickets, CONTEXT.md, or the ticket 06 ADR.

## Comments

- The user confirmed on 2026-09-05 that ADR-0006's existing web-only boundary
  should remain in force rather than be reopened.
- This ticket was written against the older full terminal-interaction-parity
  plan. The focused TUI specification has since superseded that plan and
  already places runtime overlays outside its delivery boundary.

## Answer

This ticket is superseded and sits outside the current map's destination.

ADR-0006 remains authoritative: runtime observations are an optional web-only
capability outside the terminal-parity contract. The current focused TUI
specification independently confirms that the web viewer owns runtime
observations and that runtime overlays are outside focused TUI delivery.

Therefore version one adds no eighth TUI strip channel, terminal runtime
cadence semantics, TUI runtime adapter, or runtime-specific terminal parity
gates. There is no separate parity meaning to define for a capability the TUI
does not present, and no amendment is needed to `CONTEXT.md`, ADR-0006, or the
focused TUI specification.

Any future request for TUI runtime observations must be charted as a new scope
expansion that explicitly reconsiders ADR-0006 and the focused TUI product
boundary; it is not latent parity debt in this map.
