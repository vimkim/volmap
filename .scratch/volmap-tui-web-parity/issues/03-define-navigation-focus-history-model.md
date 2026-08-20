Label: wayfinder:grilling
Type: grilling
Status: open
Assignee: unassigned
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Prototype terminal interaction parity across Volume, Sector, and Page](01-prototype-terminal-interaction-parity.md)

# Define the TUI navigation, focus, and history state model

## Question

What explicit TUI state machine governs the Volume → Sector → Page replacement hierarchy, breadcrumbs, `Enter` descent, `Esc`/`Backspace` ascent, back-stack restoration, roving page focus, independent pane scrolling, overlays, filters, findings navigation, mouse hit regions, resize, and existing sector/volume accelerators? Resolve which state survives screen changes and revision adoption, how inaccessible or filtered selections behave, and how keyboard-only and mouse operation remain equivalent at every supported terminal tier.
