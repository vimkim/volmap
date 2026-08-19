# Volmap Inspector

Volmap is a read-only offline inspector for the CUBRID `feat/oos` physical
volume format pinned at commit `e1e651debf6cc100172bde96603b17424f9c135a`.
It produces structural and allocation facts without returning database record
payloads or raw page bytes.

The implementation is under active development. Plaintext fast scans, volume
and sector maps, explicit file allocation maps, slotted-page inspection
(including role-gated E-hash buckets), OOS and `REC_BIGONE` overflow-chain
validation, heap header/chain metadata, caller-proven value-free heap object
envelopes (including MVCC header structure), validated same-heap relocation
edges, vacuum metadata, structural B-tree
root/node/OID-overflow metadata, validated catalog directory/class/representation
metadata, terminal browsing,
deterministic HTML export, and the live web viewer are usable. TDE
decryption is available from an explicit local key file and applies to every
deep page read. The pinned source-derived acceptance corpus covers the current
semantic page families. Fast page facts use an exact 16-byte canonical form and
automatically move to private, unlinked spill storage under the resident-memory
budget. Bounded envelope workers merge facts and findings back into canonical
physical order. The remaining mutation/fuzz, distribution, and company release
approval matrices are still release gates.

## Build

The supported release target is static Linux x86-64 musl:

```sh
cargo build --release --locked --target x86_64-unknown-linux-musl
```

The resulting executable is
`target/x86_64-unknown-linux-musl/release/volmap`.

## Inputs and selectors

Every inspection command takes exactly one input:

```text
--database NAME [--databases-file FILE]
--vinf PATH [--volume-root DIR]
```

Selectors are canonical nonnegative decimal identifiers:

```text
volume:VOLID
sector:VOLID:SECTORID
file:VOLID:FILEID
page:VOLID:PAGEID
slot:VOLID:PAGEID:SLOTID
oos:VOLID:PAGEID:SLOTID
```

## Examples

```sh
volmap summary --vinf /snapshot/db_vinf --format human
volmap map --vinf /snapshot/db_vinf --format json
volmap map --vinf /snapshot/db_vinf file:0:128 --format json
volmap inspect --vinf /snapshot/db_vinf page:0:129 --format human
volmap inspect --vinf /snapshot/db_vinf oos:1:2243:0 --format json
volmap tui --vinf /snapshot/db_vinf
volmap export html --vinf /snapshot/db_vinf --output report.html \
  --enrich page:0:129 --enrich oos:1:2243:0
```

`human`, `json`, and `jsonl` finite outputs share the same inspection graph.
Machine output includes snapshot identity, revision, validity, coverage,
outcome, and diagnostics. It omits input paths, source bytes, and application
values.

## Web access

Loopback is the default, and the browser opens the inspection directly without
a credential prompt:

```sh
volmap serve --vinf /snapshot/db_vinf --listen 127.0.0.1:8080
```

From another machine, forward the loopback listener through SSH:

```sh
ssh -L 8080:127.0.0.1:8080 user@server
```

To accept remote clients, explicitly listen on all IPv4 interfaces:

```sh
volmap serve --vinf /snapshot/db_vinf --listen 0.0.0.0:8080
```

This mode is deliberately unauthenticated: anyone who can reach the port can
inspect metadata and request bounded enrichment. Use a firewall, SSH, a VPN, or
a trusted TLS reverse proxy when the network is not fully trusted. Binding a
specific non-loopback address or the IPv6 wildcard is rejected; remote access
must be the explicit `0.0.0.0:PORT` form.
The volume workspace renders a progressively loaded full-volume mosaic: every
sector is an 8×8 grid of its 64 pages. Allocated slotted pages split green
occupied space from blue free space using an eager header summary; pages whose
occupancy cannot be established use a green/blue unknown pattern. Allocation
classes remain distinct, and findings are shown as a separate outline.
Selecting a sector replaces the mosaic with a large 64-page view. Selecting a
page replaces that sector with detailed structural facts. Slotted pages show an
exhaustive 16,344-byte content distribution: the slotted header, every allocated
record extent, every fragmented or contiguous free interval, and the complete
slot directory. Directory entries are colored and labeled as allocated,
unallocated, or deleted, with record and directory offsets and sizes shown
separately. Every volume, sector, page, slot, and OOS view has a canonical URL
that pins both the snapshot and immutable inspection revision. Browser Back and
Forward restore those exact views and revisions; reloading a deep URL restores
the same view directly. Enrichment publishes a new revision URL, so the
previous revision remains reachable in browser history. Breadcrumb and Back
controls return to the preceding level.

## Safety and scope

- Use a stopped database, immutable snapshot, or copy. Volmap invalidates its
  snapshot if an input changes during inspection.
- The tool never repairs or writes a CUBRID volume.
- Deep decoding is selective and resource-bounded. A stopped boundary is
  represented in coverage and diagnostics rather than silently sampled.
- Application payload bytes, decoded values, ciphertext, and TDE secrets are
  outside every output surface.

## Resource defaults

Version-one internal defaults are a 256 MiB admitted resident limit, 2 GiB
private spill limit, four envelope workers, 16,384 chain steps, and 256 MiB of
decoded input per explicit operation. The limits are hard admission boundaries,
not preallocations. Reaching one publishes the validated prefix with partial
coverage and never silently samples.

The deterministic reference-host matrix covers small, medium, large, 4 GiB
sparse, fully reserved, corruption-heavy, resident/spilled, cancellation, query,
and 512-chunk OOS profiles. Run it with `just resource-benchmark`. Its timings
are engineering evidence, not a device-independent performance SLO; cold-cache,
constrained-host, and cross-distribution runs remain release checks.

Run `volmap licenses` for the embedded project notice. The complete decision
and acceptance specification is in
`.scratch/volmap-inspector/implementation-spec.md`.
