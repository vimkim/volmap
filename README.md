# Volmap Inspector

Volmap is a read-only offline inspector for the CUBRID `feat/oos` physical
volume format pinned at commit `e1e651debf6cc100172bde96603b17424f9c135a`.
It produces structural and allocation facts without returning database record
payloads or raw page bytes.

The implementation is under active development. Plaintext fast scans, volume
and sector maps, explicit file allocation maps, slotted-page inspection, OOS
chain validation, heap header/chain metadata, vacuum metadata, terminal
browsing, deterministic HTML export, and the authenticated web viewer are
usable. TDE decryption, the remaining deep page families, the immutable
acceptance corpus, and measured production resource defaults are still release
gates.

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

Loopback is the default. The bearer token is printed once to the controlling
terminal, or written to a new mode-0600 file:

```sh
volmap serve --vinf /snapshot/db_vinf --listen 127.0.0.1:8080
```

From another machine, forward the loopback listener through SSH:

```sh
ssh -L 8080:127.0.0.1:8080 user@server
```

For an explicitly trusted internal network, plain HTTP on all interfaces is
supported with an explicit acknowledgement and exact browser origin:

```sh
volmap serve --vinf /snapshot/db_vinf \
  --listen 0.0.0.0:8080 \
  --allow-remote-http \
  --external-origin http://10.0.0.15:8080
```

This mode provides authentication, not transport confidentiality or integrity.
Use SSH, a VPN, or a trusted TLS reverse proxy when the network is not trusted.
The token is never accepted in a URL and the browser keeps it only in memory.

## Safety and scope

- Use a stopped database, immutable snapshot, or copy. Volmap invalidates its
  snapshot if an input changes during inspection.
- The tool never repairs or writes a CUBRID volume.
- Deep decoding is selective and resource-bounded. A stopped boundary is
  represented in coverage and diagnostics rather than silently sampled.
- Application payload bytes, decoded values, ciphertext, and TDE secrets are
  outside every output surface.

Run `volmap licenses` for the embedded project notice. The complete decision
and acceptance specification is in
`.scratch/volmap-inspector/implementation-spec.md`.
