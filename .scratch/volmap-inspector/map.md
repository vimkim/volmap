Label: wayfinder:map
Status: resolved

# Chart an implementation-ready route to Volmap Inspector

## Destination

An implementation-ready specification and decision index for a redesigned `volmap` successor that reads a pinned CUBRID `feat/oos` volume format offline and ships as one static Linux x86-64 executable with human and JSON CLI output, a TUI, and a remotely accessible interactive web viewer. The map is complete when no product, format, architecture, security, interaction, or verification decision remains implicit before implementation begins.

## Notes

- This map plans the implementation; it does not implement the product.
- Initial CUBRID format baseline: `/home/vimkim/gh/cb/feat-oos` at commit `e1e651debf6cc100172bde96603b17424f9c135a` (2026-08-14).
- Primary local evidence: the pinned CUBRID source and generated database fixtures. Behavioral evidence: `volmap-standalone` and `recovered/` in this repository. OOS terminology and normative behavior: `/home/vimkim/gh/cubrid-oos-context/OOS-CONTEXT.md`.
- Every session resolving a human decision should consult the `grilling` and `domain-modeling` skills. OOS decisions should also consult `cubrid-oos-context`; module-boundary decisions should consult `codebase-design`; UI decisions should use `prototype`.
- On 2026-08-19 the user directed the remaining map to use the source-backed recommended answer for every unresolved decision without further HITL. Each ticket must still research, record, and audit its recommendation; “accept all” does not waive evidence or verification.
- The inspector is strictly read-only and supports stopped databases, immutable snapshots, or copied volumes only. It fingerprints inputs and invalidates trust if they change during inspection.
- `Standalone executable` has the meaning recorded in `CONTEXT.md`: no runtime dependency on glibc, CUBRID libraries, installation assets, external web assets, or third-party network services.
- One `volmap` executable provides CLI, TUI, HTML export, and web-service modes. Both human-readable output and stable JSON CLI output are in scope.
- The full web viewer may serve data lazily. Loopback is the default; an explicit `0.0.0.0` all-interface listener is supported with mandatory token authentication. Built-in TLS is not required for version one.
- Deep inspection exposes structurally proven metadata, record boundaries, sizes, and storage references, but never application payload bytes, decoded user values, ciphertext, or TDE key material in version one.
- Prefer evidence-backed classifications over a single overloaded “status”; use the glossary in `CONTEXT.md`.
- This repository had no commits when the map was charted, so the initial research reports were isolated by distinct files under `research/` instead of unusable throwaway Git branches.
- Durable work-tracker item: `1`.
- Wayfinding completed on 2026-08-20. [The implementation specification](implementation-spec.md) is the handoff; implementation defects and release-gate execution do not reopen this decision map unless the destination itself changes.

## Decisions so far

- [Choose the implementation platform for the standalone inspector](issues/01-choose-implementation-platform.md) — Use safe Rust targeting `x86_64-unknown-linux-musl`; prove static linkage on the final ELF and retain Go as the documented fallback.
- [Reconstruct the pinned feat/oos disk-format contract](issues/02-reconstruct-pinned-disk-format.md) — Pin explicit x86-64/GCC offsets and invariants, separating native little-endian storage metadata from big-endian heap object-representation bytes.
- [Establish licensing and reverse-engineering provenance boundaries](issues/03-establish-licensing-provenance.md) — Use specifically Apache-labeled pinned sources with attribution, quarantine recovered artifacts as an authorized black-box oracle only, and require owner/counsel decisions before release.
- [Set ownership, licensing, and recovered-oracle policy](issues/14-set-ownership-license-oracle-policy.md) — CUBRID owns an internal-first Apache-2.0 project; source and generated fixtures are authoritative, recovered artifacts require separate internal approval, and public release remains company-gated.
- [Set the TDE-encrypted page inspection boundary](issues/15-set-tde-inspection-boundary.md) — Version one optionally decrypts AES/ARIA user pages only from an explicit local key file, otherwise reports them opaque; no interface exposes secrets, ciphertext, or application payloads.
- [Define the canonical inspection model and evidence levels](issues/04-define-inspection-model.md) — All adapters project one normalized, revisioned inspection graph with snapshot-scoped identities, traceable evidence, orthogonal availability/coverage/diagnostics, complete fast topology, and targeted deep page/OOS enrichment.
- [Define corruption containment and diagnostic semantics](issues/10-define-corruption-diagnostics.md) — Fail closed at hierarchical validation boundaries while preserving independently valid facts; use stable cataloged diagnostics, explicit coverage ledgers, deterministic cross-adapter projection, and nonzero outcomes for corruption or unexpected incompleteness.
- [Choose the scan, index, and cache architecture](issues/05-choose-scan-index-cache-architecture.md) — Use one revision-aware inspection module over a virtual packed graph, bounded memory and session-only spill, deterministic complete fast scans, explicit deep enrichment, immutable revisions, and fingerprinted concurrent reads.
- [Define the CLI and JSON contract](issues/06-define-cli-json-contract.md) — Use explicit intent commands and inputs, typed selectors, non-scriptable human rendering, versioned complete JSON/JSONL projections, canonical diagnostic/coverage framing, and stable outcome-aware exit statuses.
- [Define the web service, remote-access, and HTML-export contract](issues/07-define-web-service-security-export.md) — Serve authenticated revision-pinned JSON over loopback/SSH or explicitly enabled internal all-interface HTTP, enforce exact browser origins and bounded requests without exposing raw bytes, and produce deterministic bounded self-contained HTML for one frozen revision.
- [Prioritize version-one page-type decoders](issues/13-prioritize-page-decoders.md) — Decode allocation, heap/OOS/overflow, minimal catalog, structural B-tree, and vacuum metadata without application values; give E-hash only role-gated slot maps, keep transient/specialized bodies opaque, and require an immutable pinned fixture matrix.
- [Prototype the web sector and slotted-page explorer](issues/08-prototype-web-explorer.md) — Use a bounded full-volume mosaic, then replacement drill-down into a large 64-page sector and a detailed page workspace with an exhaustive record/free-space/slot-directory distribution, breadcrumbs, and Back navigation, while keeping raw bytes and payloads outside every browser projection.
- [Prototype the TUI navigation and inspection flow](issues/09-prototype-tui-flow.md) — Use a responsive stacked 64-page map and tabbed inspector, with an optional hierarchy overlay, typed selectors, complete keyboard parity, and graceful 160×45 through 80×24 behavior.
- [Evaluate the final Rust dependency and reproducible-release graph](issues/16-evaluate-rust-release-graph.md) — Pin Rust 1.97.1 and a permissive pure-Rust musl graph, keep adapters behind one inspection seam, embed all runtime assets/notices, allow Cargo to fetch locked sources during builds, and require byte-identical static builds plus SBOM/license gates.
- [Define validation oracles, fixtures, and acceptance gates](issues/11-define-validation-oracles.md) — Treat pinned layout probes and immutable generated fixtures as authority, add deterministic corruptions/property/fuzz and cross-adapter non-disclosure tests, restrict recovered parity to approved legacy facts, and gate release on static reproducible cross-distribution execution.
- [Assemble the implementation-ready specification and delivery sequence](issues/12-assemble-implementation-spec.md) — Use one referenced specification and dependency-ordered delivery checklist; distinguish closed product decisions from measurement and external-approval release gates.
- [Measure representative scan performance and set resource-budget defaults](issues/17-measure-scan-resource-budgets.md) — Use measured internal defaults of 256 MiB resident memory, 2 GiB private spill, four workers, 16,384 chain steps, and 256 MiB decoded input while keeping host/distribution confirmation as release gates rather than universal SLOs.

## Not yet specified

None.

## Out of scope

- Inspecting a running database or promising a transactionally consistent view of changing volumes.
- Modifying, repairing, compacting, or otherwise writing database volumes.
- Initial compatibility with moving `feat/oos`, older CUBRID releases, non-Linux operating systems, or non-x86-64 architectures.
- Automatic semantic decoding or display of application record values.
- Built-in TLS termination or safe direct exposure to an untrusted public network in version one; use SSH, VPN, or a trusted reverse proxy.
- Producing the implementation itself within this Wayfinder effort.
