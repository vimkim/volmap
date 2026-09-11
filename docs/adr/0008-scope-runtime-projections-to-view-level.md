---
status: accepted
---

# Scope runtime projections to the inspection view

Volume runtime requests identify a volume and bounded sectors, and return only residency classification and LRU membership/zone from one shared capture. Sector requests identify one sector; selected-page requests retain their separate detail purpose. This avoids repeating thousands of page identifiers and shipping selected-page evidence for a volume mosaic, preserving the bounded broker and HTTP budgets while expanding simultaneous display to 4,096 pages.

The user confirmed the design on 2026-09-11: Volume selects up to 64 visible sectors nearest the viewport centre, marks excess sectors unevaluated, and does not rotate them on a timer. Sector retains its existing detailed runtime fields for 64 pages. Partial omissions remain unknown, capture identity does not imply atomicity, and the pause/expiry/incarnation rules of ADR-0006 remain in force.

The [design](../../.scratch/volume-overlay-64-sectors/spec.md) retains the 64 KiB request and 1 MiB response ceilings through compact scope addressing and the slim Volume projection. Its reproducible JSON size model supports feasibility; implementation, allocation accounting and multi-tab/browser performance gates remain to be verified.
