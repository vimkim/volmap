Label: wayfinder:task
Type: task
Status: resolved
Assignee: codex
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Define the TUI parity verification contract](08-define-parity-verification-contract.md)

Working agreement: Resolve only this final assembly ticket in the sibling worktree `/home/vimkim/temp/volmap-tui-redesign-spec` on branch `wayfinder/tui-redesign-spec`; preserve tickets 01–08 as the authoritative decisions, identify rather than silently settle any remaining contradiction, and squash the audited documentation result onto local `main` without including unrelated concurrent changes.

# Assemble the implementation-ready TUI redesign specification

## Question

After every product, interaction, shared-interface, state, rendering, resource, accessibility, and verification decision on this map is resolved, assemble one implementation-ready specification that links rather than duplicates those decisions. Include the accepted view/state model, module seams, invariants, adaptive layouts, enrichment lifecycle, rendering semantics, compatibility constraints, verification gates, and a dependency-ordered delivery sequence suitable for handoff to `/to-tickets`; identify any remaining unresolved item instead of silently choosing it during assembly.

## Answer

The accepted decisions are assembled in the [Volmap TUI terminal-parity implementation specification](../implementation-spec.md). It is an implementation index rather than a second decision source: each concern links to its authoritative ticket, while the cross-module invariants and D0–D8 dependency-ordered delivery packages give `/to-tickets` an executable handoff with an explicit completion criterion for every package.

The assembly audit found no unresolved product, interaction, shared-interface, state, rendering, resource, accessibility, compatibility, or verification decision. It records the only apparent precedence point instead of choosing silently: the fixed seven-column Page strip in [Define semantic color, glyph, and fallback mappings](06-define-semantic-terminal-rendering.md) is the later semantic constraint, and [Set volume viewport and rendering resource budgets](07-set-viewport-resource-budgets.md) already applies it to replace the accepted prototype's illustrative narrow-card geometry without replacing Atlas's hierarchy.

The delivery sequence begins with the parity corpus/matrix, then establishes the Projection workspace; AtlasMachine and the semantic renderer can develop across their accepted seams before screen integration, enrichment, viewport/resource enforcement, terminal-host integration, and one-candidate release/cutover. Production implementation, legacy deletion, and creation of delivery tickets remain outside this Wayfinder session.

No prerequisite ticket was modified, no new ticket or domain term was required, and [`CONTEXT.md`](../../../CONTEXT.md) remains the authoritative glossary.
