Label: wayfinder:grilling
Type: grilling
Status: resolved
Assignee: codex
Parent: [Chart terminal interaction parity for the Volmap TUI](../map.md)
Blocked by: None

# Define the shared projection boundary for terminal parity

## Question

What deep module and interface should supply both web and TUI adapters with page occupancy, exhaustive slotted-page distribution, safe slot-directory facts, and bounded deep-enrichment results without coupling the TUI to HTTP/browser machinery or duplicating the web-private distribution calculation? Decide ownership of derived geometry, query/session state, immutable revisions, cancellation, diagnostics, and adapter formatting; identify which current web behavior and tests form compatibility gates.

## Answer

### Decision

The user accepted the recommended minimal hybrid at source baseline `cba72cd` (`feat: attribute pages and sectors to their table in web, TUI, and JSON`). Introduce one deep **Projection workspace** module at the seam above the inspection graph and below the web and TUI inspection adapters. It replaces neither the inspection graph nor either adapter: it hides immutable-revision retention, presentation-neutral derivation, and enrichment arbitration behind one small interface.

The external interface has three operations:

```rust
checkout(exact: RevisionKey) -> Result<RevisionView, ProjectionError>

project(
    revision: &RevisionView,
    query: ProjectionQuery,
) -> Result<ProjectionFrame, ProjectionError>

enrich(
    base: &RevisionView,
    target: DeepInspectionTarget,
    policy: ResourcePolicy,
    cancel: &CancelToken,
    progress: Option<&mut dyn EnrichmentObserver>,
) -> Result<EnrichmentCompletion, EnrichmentError>
```

Constructing the workspace returns its initial exact `RevisionView`; only `checkout` and a successful `EnrichmentCompletion` produce other handles. There is no query that silently substitutes “latest.” `ProjectionQuery` is a closed typed sum covering the existing overview, bounded collection, Volume, Sector, Page, diagnostics, coverage, relationship, file, Slot, and OOS facts. `ProjectionFrame` carries its exact snapshot/revision, validity, outcome, coverage, typed diagnostics, and typed result together so facts from different revisions cannot be joined accidentally.

`project` is a pure read over committed facts and never reads volume bytes. `enrich` is a synchronous cooperative operation; the web and TUI adapters schedule it on their own worker/event-loop machinery. This keeps Tokio, HTTP jobs, terminal polling, and widget state outside the shared interface while centralizing the semantics both adapters must agree on.

### Ownership at the seam

| Concern | Projection workspace | Inspection adapter |
| --- | --- | --- |
| Derived facts | Allocation, known/unknown occupancy, file/class/table attribution, exhaustive Page byte-map geometry, safe Slot states, deterministic ordering and bounded windows | No semantic derivation |
| Revision queries | Exact immutable revision lookup and revision-pinned projection frames | The revision currently displayed and whether to adopt a returned revision |
| Enrichment | Resource policy, one-work admission, base/head validation, cooperative cancellation checks, progress facts, atomic publication, immutable history, terminal invalidation | Worker scheduling, visible loading state, the user's cancel action, and ignoring a late result no longer relevant to the active request |
| Diagnostics | Typed occurrences: stable code, severity, affected Entity references, evidence locus, containment impact, and coverage | HTTP/TUI wording, emphasis, and transport mapping; never infer behavior from message or subject text |
| Query state | Typed identities and deterministic collection windows | Selectors, cursor encoding/signing, selected entity, focus, filters, search, scroll, tabs, and navigation history |
| Presentation | Semantic enums, counts, ranges, and availability | JSON/JSONL schema mapping, HTTP status and URLs, browser history, CSS/ARIA, ANSI/Unicode/ASCII, layout, truncation, and mouse hit regions |

The existing `Live inspection session` remains the web-serving process and its HTTP/browser state. It becomes a web adapter over the Projection workspace rather than the name or interface of the shared module. CLI-human, JSON/JSONL, and deterministic HTML behavior remain unchanged; they may keep their current projection path until adopting the new seam is useful.

### Shared projection contracts

- A Sector contains exactly 64 Pages in ascending physical Page order. Bounded Sector windows are contiguous and exhaustive when followed; sampling or omission is forbidden.
- Page occupancy remains `unknown` or `known { occupied_percent, free_percent }`; unknown is never converted to zero. The existing validated 15-level quantization, including the current 7/93 minimum known bucket, remains compatible.
- Page and Sector file/class/table attribution introduced by `cba72cd` is shared semantic data, not a label invented by either adapter. Preserve Page states `none`, `mixed-claims`, `allocated`, and `reserved-for`; Sector states `unclaimed`, `single`, and `mixed`; typed file role, class OID, and resolved/unresolved/not-applicable class-name outcomes remain distinct.
- One shared Page result atomically contains its Page facts, deep-detail state, complete safe Slot directory, and Page byte map for the same revision. An adapter never joins these from separate revisions.
- An available slotted Page byte map covers the 16,344-byte Page content exactly: header `[0, 32)`, live record extents ordered by `(offset, slot_id)`, every fragmented or contiguous free interval, and the complete four-byte-per-entry Slot directory ending at the content boundary. Header, records, free regions, and Slot directory sum to the content size with no overlap or omission.
- Every Slot directory entry is present in Slot-id order and retains its structural record type and `allocated`, `unallocated`, or `deleted` state. Empty/deleted entries have no live record extent. Geometry is derived only from a fully validated slotted-page structure; `GraphView`, `SlottedPage`, format constants, raw bytes, application payload, ciphertext, keys, and source paths never cross the seam.
- Availability, Page detail support, inspection coverage, TDE inspection state, diagnostic severity, containment impact, and inspection outcome remain separate dimensions.

### Immutable revisions, enrichment, and cancellation

The workspace retains every published immutable revision for the process lifetime and records one writable head. Enrichment accepts only the current usable head, checks that fact before work and again immediately before publication, and serializes admission initially to match current web resource behavior. A stale base, wrong snapshot, already-invalidated snapshot, unsupported target, or refused admission is a typed transport-neutral error.

The old `RevisionView` never changes. `EnrichmentCompletion` returns `Published { revision }` or `Unchanged { revision }`; an adapter explicitly adopts that handle only if the request is still active. Completion never mutates an adapter's displayed revision. Web job ids, `202`/`Location`, result URLs, and TUI status text are adapter formatting over this completion.

Cancellation is cooperative and idempotent at validation boundaries. Preserve the inspection graph's target-specific publication semantics: Page interruption before a publishable result returns cancellation without a revision, while bounded chain work may publish its validated prefix with partial coverage and an `interrupted` or resource-limit stop diagnostic. A cancel/publication race has one terminal semantic result; cancellation never hides an already-published revision. Progress reports evaluated and conclusive counts with only trusted totals—never guessed percentages.

Structural decode failure, source invalidation discovered during enrichment, or a bounded validated prefix may therefore produce a new diagnostic-bearing revision rather than a generic operation failure. Transport-neutral error variants must at least distinguish wrong snapshot, revision/entity not found, stale base with current head, snapshot invalidated, unsupported, admission refused, cancelled before publication, and internal source/fact-store/arithmetic failure.

### Compatibility gates

Implementation must preserve observable behavior at `cba72cd` and move semantic assertions to the new interface without weakening adapter-level regressions:

- Keep `inspection_opens_sparse_volume_and_scans_only_reserved_sector_envelopes` as the allocation, exact 7/93 occupancy, fail-closed Page association, unclaimed Sector attribution, and 64-Page baseline.
- Move `slotted_page_distribution_covers_records_free_space_and_directory_entries` beside the shared geometry implementation. Preserve its exact 16,344-byte partition, ordered records, fragmented and contiguous free regions, directory position, allocated/unallocated/deleted entries, and byte-conservation assertion.
- Keep `source_mutation_terminally_invalidates_new_work_without_rewriting_old_view`, `deep_page_enrichment_publishes_a_new_revision_without_rewriting_the_old_view`, and the complete/budget-stopped OOS enrichment tests as revision-publication gates. Add explicit Page-cancel/no-publication and chain-cancel/validated-prefix tests.
- Keep the file-table descriptor tests for heap, overflow, B-tree, hash, null, and partial class associations. Add shared-interface fixtures covering all Page and Sector attribution states and assert that web and TUI receive identical typed attribution before formatting.
- Keep the diagnostic catalog and outcome-precedence tests; add an interface test proving navigation uses affected Entity references rather than parsing diagnostic subject strings.
- Keep the web browser contracts for the complete Volume mosaic, 64-Page Sector, replacement-screen Page drill-down, known/unknown occupancy, exhaustive distribution, revision-pinned history, bounded Sector pagination, revision-bound cursors, structured conflicts, and terminal invalidation overlay.
- Add exact web Page-resource JSON tests before and after enrichment, including the additive `file_association` and Sector `attribution` shapes from `cba72cd`. Preserve current `202` completion, result revision/location, stale-base and invalidation conflicts, resource refusal, unsupported target, and diagnostic-bearing decode-failure behavior.
- Keep `json_document_is_revision_pinned_and_path_free` and existing JSON/JSONL ordering/schema gates. Shared refactoring must not change schema version 1, current field values, disclosure behavior, or deterministic HTML output.

Tests should exercise behavior through the Projection workspace interface; private geometry and revision-store tests that duplicate those assertions should be removed rather than layered underneath. Web and TUI adapter tests then verify only their transport, interaction, and rendering mappings.

### Consequences

Deleting this module would force occupancy/attribution joins, slotted geometry, revision arbitration, cancellation, invalidation, and diagnostic navigation back into both interactive adapters, so the seam earns its place. The closed query sum is intentionally less extensible than a recipe algebra and less hierarchy-shaped than nested Atlas handles: adding a genuinely new fact requires extending the shared result types, while the common Volume → Sector → Page caller remains direct and hard to mix across revisions.

No new ticket is created by this decision. The migration-shape fog is narrower but still depends on [Choose the terminal rendering architecture and dependency boundary](04-choose-rendering-architecture.md); terminal compatibility fog still requires concrete rendering failures before it can become a precise ticket.
