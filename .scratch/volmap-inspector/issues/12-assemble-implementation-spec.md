Type: task
Status: resolved
Blocked by: 01, 02, 03, 04, 05, 06, 07, 08, 09, 10, 11, 13, 14, 15, 16

# Assemble the implementation-ready specification and delivery sequence

## Question

Consolidate the closed decision tickets by reference into one implementation-ready specification without duplicating their rationale. Verify that the format profile, domain model, module interfaces, scan behavior, CLI/JSON/TUI/web contracts, remote security boundary, page and OOS visualization, diagnostic semantics, licensing obligations, acceptance gates, and static distribution strategy contain no unresolved decisions. Then order implementation slices by dependency and verification value, explicitly keeping implementation outside this Wayfinder map.

## Comments

### Resolution

The consolidated specification is [implementation-spec.md](../implementation-spec.md). It
references the owning decision tickets instead of copying their rationale, fixes module
interfaces and dependency order, and identifies fixture-dependent release gates without
turning missing evidence into invented defaults. The specification was audited after all
listed blockers were resolved. Ticket 17 remains a separate measurement gate and therefore
does not block this consolidation ticket.

## Answer

Use [implementation-spec.md](../implementation-spec.md) as the implementation and release
checklist. No human product decision remains open; remaining TBD values are measurements or
external release approvals with an explicit gate and owner.
