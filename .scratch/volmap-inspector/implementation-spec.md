# Volmap Inspector implementation specification

Status: implementation-ready

Format authority: CUBRID `e1e651debf6cc100172bde96603b17424f9c135a`

Target: one static `x86_64-unknown-linux-musl` executable

## Contract index

The owning tickets are normative and retain the evidence and rationale:

- Platform, release graph, and provenance: [01](issues/01-choose-implementation-platform.md), [03](issues/03-establish-licensing-provenance.md), [14](issues/14-set-ownership-license-oracle-policy.md), [16](issues/16-evaluate-rust-release-graph.md).
- Physical format and supported structures: [02](issues/02-reconstruct-pinned-disk-format.md), [13](issues/13-prioritize-page-decoders.md), [15](issues/15-set-tde-inspection-boundary.md).
- Canonical graph, diagnostics, and execution: [04](issues/04-define-inspection-model.md), [05](issues/05-choose-scan-index-cache-architecture.md), [10](issues/10-define-corruption-diagnostics.md).
- Adapters and interaction: [06](issues/06-define-cli-json-contract.md), [07](issues/07-define-web-service-security-export.md), [08](issues/08-prototype-web-explorer.md), [09](issues/09-prototype-tui-flow.md).
- Acceptance and budgets: [11](issues/11-define-validation-oracles.md), [17](issues/17-measure-scan-resource-budgets.md).

## Hard boundaries

- Inputs are stopped databases, immutable snapshots, or copies. All volume files are opened read-only and fingerprinted; a changed source invalidates the inspection revision.
- The only supported format is the pinned Linux x86-64 GCC profile. Native storage metadata is decoded at explicit little-endian offsets; heap object representation uses its separately pinned encoding.
- Version one emits structural facts, allocation, record extents, and validated internal links. It never emits source bytes, ciphertext, keys, nonces, application payloads, or decoded user values.
- Corruption is contained at the smallest validated boundary. Every result carries independent validity, availability, coverage, diagnostics, and outcome rather than an overloaded status.
- Every adapter is a projection of one immutable revisioned graph. Adapters do not parse volume bytes or invent classifications.

## Module seams

```text
source discovery + read-only positional I/O
        |
        v
pinned checked decoders  ---> canonical diagnostics
        |
        v
inspection session: fast scan -> immutable revision N
        |                         |
        |                         +-> selective page/file/OOS enrichment -> N+1
        v
stable projections -> CLI/JSON/JSONL | TUI | HTML export | HTTP API/browser
```

- `source` owns strict database/`_vinf` discovery, remapping containment, file stamps, and complete positional reads.
- `format` owns byte bounds, arithmetic, format constants, and context-specific decoders. It returns typed facts or stable rules and never logs or allocates from unchecked disk counts.
- `inspection` owns admission, cancellation, scan order, reconciliation, graph revisions, coverage, and cross-page traversal. It is the only module allowed to combine decoder facts.
- `projection` owns disclosure-safe stable schema version 1 and canonical ordering.
- `cli`, `tui`, `export`, and `web` are adapters only.

## Scan and enrichment

1. Discover the explicit input and open all data volumes read-only.
2. Decode page-zero volume headers, validate geometry and identities, then decode allocation bitmaps.
3. Fast-scan the 32-byte prefix and 8-byte watermark of every page in reserved sectors in physical order. Never silently sample.
4. Re-read file stamps before publication. Publish revision zero with topology, page envelopes, coverage, and diagnostics.
5. Decode page bodies only for an explicit target. File-table header interpretation requires an explicit `file:` selector because continuation pages share `PAGE_FTAB`.
6. Traverse file tables and OOS/overflow links with cycle, target-role, step, decoded-byte, and memory checks. Publish validated prefixes when the contract permits partial results.
7. Recheck file stamps before publishing every later immutable revision. Older revisions remain queryable for the session lifetime.

The packed store, worker count, and spill behavior follow ticket 05. Ticket 17's measured internal defaults are 256 MiB admitted resident memory, 2 GiB private spill, four workers, 16,384 chain steps, and 256 MiB decoded input. Limits publish an explicit partial prefix rather than sampling; cold-cache, constrained-host, and cross-distribution confirmation remains a release gate.

## Page-family delivery boundary

Version-one semantic or structural support follows ticket 13:

- Allocation: volume header/bitmap, file header and partial/full/user allocation tables, tracker inventory.
- Records: common slotted pages and record types, heap chain/header and MVCC envelope, `REC_BIGONE` overflow chains, OOS chunk chains.
- Metadata: minimal catalog representation metadata, structural B-tree nodes/OID overflow/key extents, vacuum and dropped-file chains.
- Role-gated only: extendible-hash slot maps.
- Opaque: query-result/application values, temporary area bodies, log files outside volume input, unsupported or encrypted bodies without keys.

Each decoder must name its prerequisite context. Reserved enum values are reported as reserved, not automatically corrupt. Every count, offset, length, recursion, and link traversal is bounded before allocation or following an edge.

## Adapter contracts

- CLI grammar, selectors, schema, channel separation, and exit statuses are fixed by ticket 06.
- The TUI uses the accepted 64-page grid, hierarchy, tab, jump, keyboard/mouse parity, and terminal-restoration behavior from ticket 09.
- HTML export freezes exactly one revision, is deterministic and self-contained, refuses overwrite, writes privately via atomic publication, and performs no network requests.
- The live browser presents the selected full volume as bounded, progressively loaded sector cards. Every card projects exactly 64 physical-order page squares; allocation class is the base color and findings use an independent outline/text treatment. Continuation is snapshot/revision/volume-bound and one response is capped at 64 sectors.
- Live navigation replaces the center workspace in three stages: volume mosaic, enlarged 64-page sector, and enlarged page detail. Breadcrumb and Back controls preserve ancestry. Selecting a page is an explicit bounded enrichment request when supported detail is absent; the returned revision must be adopted before rendering structural facts. A slotted-page response projects the exact 16,344-byte content geometry: its 32-byte header, every live record extent, the complete complement of fragmented/contiguous free intervals, the trailing four-byte-per-entry slot directory, and every directory entry classified as allocated, unallocated, or deleted. The browser combines a truthful full-page ruler with magnified interval rows and a complete clickable directory; it exposes offsets and sizes but no source bytes or payload values.
- HTTP defaults to numeric loopback. Plain HTTP on a non-loopback numeric listener is allowed only with `--allow-remote-http`; wildcard binding additionally requires an exact externally visible origin. This supports SSH forwarding and explicitly accepted trusted internal `0.0.0.0` operation without claiming transport security.
- HTTP API requests require the per-run bearer token. Mutable work requests additionally require exact JSON content type and exact Origin; no CORS headers are emitted. Host authority, URI/header/body size, and concurrency are bounded. Security headers apply to every response.
- Browser tokens live only in memory. No URL, fragment, local/session storage, export, log, or response contains them.

## TDE

Without a key file, encrypted bodies remain `encrypted-opaque`; their plaintext envelope and allocation ownership remain inspectable. Optional decryption is only through an explicit local `--tde-keys-file`. Key-file parsing, master-key selection, permanent-key unwrap, AES/ARIA CTR decoding, secret zeroization, and the synthetic encrypted fixture matrix are a release gate under ticket 15. No web endpoint accepts or returns key information.

## Verification and release gate

A releasable build requires all of the following:

- Immutable fixtures generated by the pinned source commit for every supported family, including synthetic corruption and AES/ARIA cases, with provenance and hashes.
- Unit, property, mutation, fuzz, cross-adapter parity, non-disclosure, deterministic-export, hostile-request, cancellation, and source-mutation tests from ticket 11.
- Ticket 17's small/medium/large/sparse/dense/corrupt/OOS benchmark matrix, measured numeric defaults, and proof that resource exhaustion changes coverage explicitly rather than sampling.
- Locked dependencies fetched by Cargo as needed, embedded notices/SBOM, two byte-identical release builds, static-link proof, and execution on the supported Linux distribution matrix. Build-time network access is allowed; runtime portability is proved from the final ELF and distribution tests.
- Recorded internal ownership/legal approval before any public release. Recovered artifacts remain excluded unless separately approved.

## Dependency-ordered implementation slices

1. Checked bytes, identifiers, source discovery, format profile, and volume/bitmap envelopes.
2. Canonical graph, diagnostics, fast scanner, source-stability invalidation, projections, CLI summary/map.
3. Slotted pages, file allocation tables, heap/OOS/overflow, then catalog/B-tree/vacuum decoders with immutable revisions.
4. TDE key path and encrypted fixtures.
5. TUI, deterministic HTML, authenticated HTTP/API/browser over the same projections.
6. Packed/spill/worker paths, representative benchmarks, fixed numeric defaults, fuzz/corruption hardening.
7. Reproducible static release, notices/SBOM, distribution matrix, and release approval.

Slices may ship internally as checkpoints, but every item in the verification section remains mandatory for a version-one release claim.
