Label: wayfinder:map

# Chart an implementation-ready route to Volmap Inspector

## Destination

An implementation-ready specification and decision index for a redesigned `volmap` successor that reads a pinned CUBRID `feat/oos` volume format offline and ships as one static Linux x86-64 executable with human and JSON CLI output, a TUI, and a remotely accessible interactive web viewer. The map is complete when no product, format, architecture, security, interaction, or verification decision remains implicit before implementation begins.

## Notes

- This map plans the implementation; it does not implement the product.
- Initial CUBRID format baseline: `/home/vimkim/gh/cb/feat-oos` at commit `e1e651debf6cc100172bde96603b17424f9c135a` (2026-08-14).
- Primary local evidence: the pinned CUBRID source and generated database fixtures. Behavioral evidence: `volmap-standalone` and `recovered/` in this repository. OOS terminology and normative behavior: `/home/vimkim/gh/cubrid-oos-context/OOS-CONTEXT.md`.
- Every session resolving a human decision should consult the `grilling` and `domain-modeling` skills. OOS decisions should also consult `cubrid-oos-context`; module-boundary decisions should consult `codebase-design`; UI decisions should use `prototype`.
- The inspector is strictly read-only and supports stopped databases, immutable snapshots, or copied volumes only. It fingerprints inputs and invalidates trust if they change during inspection.
- `Standalone executable` has the meaning recorded in `CONTEXT.md`: no runtime dependency on glibc, CUBRID libraries, installation assets, external web assets, or third-party network services.
- One `volmap` executable provides CLI, TUI, HTML export, and web-service modes. Both human-readable output and stable JSON CLI output are in scope.
- The full web viewer may serve data lazily. Loopback is the default; an explicit `0.0.0.0` all-interface listener is supported with mandatory token authentication. Built-in TLS is not required for version one.
- Deep inspection exposes structurally proven metadata, record boundaries, sizes, and storage references, but never application payload bytes, decoded user values, ciphertext, or TDE key material in version one.
- Prefer evidence-backed classifications over a single overloaded “status”; use the glossary in `CONTEXT.md`.
- This repository had no commits when the map was charted, so the initial research reports were isolated by distinct files under `research/` instead of unusable throwaway Git branches.
- Durable work-tracker item: `1`.

## Decisions so far

- [Choose the implementation platform for the standalone inspector](issues/01-choose-implementation-platform.md) — Use safe Rust targeting `x86_64-unknown-linux-musl`; prove static linkage on the final ELF and retain Go as the documented fallback.
- [Reconstruct the pinned feat/oos disk-format contract](issues/02-reconstruct-pinned-disk-format.md) — Pin explicit x86-64/GCC offsets and invariants, separating native little-endian storage metadata from big-endian heap object-representation bytes.
- [Establish licensing and reverse-engineering provenance boundaries](issues/03-establish-licensing-provenance.md) — Use specifically Apache-labeled pinned sources with attribution, quarantine recovered artifacts as an authorized black-box oracle only, and require owner/counsel decisions before release.
- [Set ownership, licensing, and recovered-oracle policy](issues/14-set-ownership-license-oracle-policy.md) — CUBRID owns an internal-first Apache-2.0 project; source and generated fixtures are authoritative, recovered artifacts require separate internal approval, and public release remains company-gated.
- [Set the TDE-encrypted page inspection boundary](issues/15-set-tde-inspection-boundary.md) — Version one optionally decrypts AES/ARIA user pages only from an explicit local key file, otherwise reports them opaque; no interface exposes secrets, ciphertext, or application payloads.

## Not yet specified

- Quantitative performance, memory, index-size, and latency budgets need representative database measurements and the selected scan architecture.
- Implementation phase sizing depends on the closed architecture, interface, prototype, and verification decisions.

## Out of scope

- Inspecting a running database or promising a transactionally consistent view of changing volumes.
- Modifying, repairing, compacting, or otherwise writing database volumes.
- Initial compatibility with moving `feat/oos`, older CUBRID releases, non-Linux operating systems, or non-x86-64 architectures.
- Automatic semantic decoding or display of application record values.
- Built-in TLS termination or safe direct exposure to an untrusted public network in version one; use SSH, VPN, or a trusted reverse proxy.
- Producing the implementation itself within this Wayfinder effort.
