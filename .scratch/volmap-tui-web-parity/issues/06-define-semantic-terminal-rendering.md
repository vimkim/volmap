Label: wayfinder:grilling
Type: grilling
Status: open
Assignee: unassigned
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: [Prototype terminal interaction parity across Volume, Sector, and Page](01-prototype-terminal-interaction-parity.md)

# Define semantic color, glyph, and fallback mappings

## Question

How should every web semantic used by the three parity views map to terminal presentation without collapsing independent dimensions? Define allocation classes, known occupied/free percentage, unknown occupancy, physical page type, findings, focus, selection, slotted header, live record extents, fragmented and contiguous free regions, slot directory, and allocated/empty/deleted slots across ANSI color, Unicode block-glyph, monochrome, and ASCII modes. Resolve precedence, legends, textual labels, Unicode display-column measurement, terminal-control sanitization for source-derived labels such as class/table names, contrast, and what information may compact—but never disappear—at 80×24 and 60×20.
