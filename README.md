# Volmap

See what is inside a CUBRID volume without starting the database or changing a
byte.

Volmap turns an offline volume set into a navigable map of Volumes, Sectors,
Pages, Files, and records. Start with the terminal interface, query the same
facts from the CLI, or explore them in a browser.

## See it in action

### Terminal

Use the arrow keys to move, **Enter** to inspect, and **Esc** or **Backspace**
to return.

![Drilling from a Volume into a Sector and Page in the Volmap TUI](docs/images/tui-demo.gif)

### Web

The browser viewer presents the same inspection facts with shareable entity
URLs and a full-volume mosaic.

![Inspecting a slotted Page in the Volmap web viewer](docs/images/slotted-page-view.png)

## Quick start

The repository's `just` recipes pin Rust 1.97.1 and the frontend toolchain. The
supported release artifact is a static Linux x86-64 musl binary:

```sh
just build-release
```

Open a stopped database registered with CUBRID:

```sh
./target/x86_64-unknown-linux-musl/release/volmap tui --database demodb
```

Or inspect a copied volume set directly through its volume-info file:

```sh
./target/x86_64-unknown-linux-musl/release/volmap \
  tui --vinf /snapshot/demodb_vinf
```

Every command accepts exactly one of these inputs:

```text
--database NAME [--databases-file FILE]
--vinf PATH [--volume-root DIR]
```

The CUBRID `develop` format is selected by default. Use
`--format-profile feat-oos` when inspecting volumes created by the experimental
OOS branch; Volmap cannot infer this choice reliably from the volume header.

## What can Volmap inspect?

- Volume geometry, Sector reservation, and Page allocation
- File ownership and Page-to-table association
- Slotted-page structure, free space, slots, and record layout
- Heap, B-tree, catalog, vacuum, and `REC_BIGONE` overflow metadata
- OOS metadata and chains under the `feat-oos` format profile
- Validated relocation and overflow chains
- TDE-encrypted Pages when an explicit local key file is supplied
- Typed record values only when the operator explicitly selects that record

Unsupported, encrypted, malformed, or incomplete evidence remains visible as
a typed diagnostic. Volmap does not guess missing facts.

## Other ways to inspect

Get a quick human-readable summary:

```sh
volmap summary --vinf /snapshot/demodb_vinf --format human
```

Query a specific entity as JSON:

```sh
volmap inspect --vinf /snapshot/demodb_vinf page:0:129 --format json
```

Start the live web viewer on loopback:

```sh
volmap serve --vinf /snapshot/demodb_vinf --listen 127.0.0.1:8080
```

The optional CUBRID page-buffer observation capability requires explicit opt-in
and a socket path together:

```sh
volmap serve --vinf /snapshot/demodb_vinf --listen 127.0.0.1:8080 \
  --runtime-page-buffer --runtime-socket /run/user/1000/cubrid/inspector.sock
```

The viewer verifies the producer's database and volume identities before supplying
observations. Select a Page and click **Enable observations** to attach.

Runtime attachment accepts loopback or an explicit IPv4 listener, such as
`--listen 192.168.4.2:7777`; wildcard runtime listeners are rejected. Direct LAN
serving is unauthenticated plain HTTP, so anyone who can reach the port can use
the enabled observations. SSH forwarding remains available for loopback serving.
There is no socket discovery or environment fallback.

For the local inspector-contract demodb setup, run `just user::serve-demodb-overlay`
and open `http://192.168.4.2:7777`. An optional positional argument overrides the
producer socket. This serves real database volumes; `serve-overlay-preview` and
`check-overlay` use a synthetic fixture preview instead.

The capability endpoint uses its own bounded admission and returns sanitized,
non-cacheable metadata; it never changes inspection facts, outcomes, revisions,
TUI or exports. Page-buffer observations, when implemented, will not prove
memory/disk correspondence, commit visibility, durability or event history.

Or create a deterministic, self-contained HTML report:

```sh
volmap export html --vinf /snapshot/demodb_vinf --output report.html \
  --enrich page:0:129
```

Selectors use nonnegative decimal identifiers:

```text
volume:VOLID
sector:VOLID:SECTORID
file:VOLID:FILEID
page:VOLID:PAGEID
slot:VOLID:PAGEID:SLOTID
oos:VOLID:PAGEID:SLOTID
```

## Safety and scope

- Volmap is read-only. It never repairs or writes a CUBRID volume.
- Finite commands require a stopped database, immutable snapshot, or stable
  copy. If an input changes during inspection, Volmap invalidates the snapshot.
- `serve` follows on-disk changes by default. It reports observed disk state,
  which is not the same as transactionally committed database state. Use
  `--no-follow` for one immutable reading.
- Optional runtime-observation sources are additive. If a source is absent,
  refuses attachment, or is unsupported, Volmap continues ordinary disk
  inspection with no runtime overlay.
- Raw application bytes, ciphertext, and TDE secrets never appear in output.
  Decoded values appear only for explicitly selected records.
- The web server has no built-in authentication. Keep it on loopback or place
  remote access behind SSH, a VPN, firewall, or trusted TLS reverse proxy.
- This project is under active development and targets two pinned CUBRID volume
  formats: `develop` commit `cd593bcf2d8643b4698f1cb311c4c23af23a9d57`
  (the default) and `feat/oos` commit
  `e1e651debf6cc100172bde96603b17424f9c135a` (selected with
  `--format-profile feat-oos`). A selected profile is never changed
  automatically; contradictory evidence may produce a diagnostic suggesting an
  explicit retry with the other profile.

The disclosure policy is documented in
[ADR-0001](docs/adr/0001-explicit-target-disclosure.md). See
[CONTEXT.md](CONTEXT.md) for the project vocabulary and
[docs/](docs/) for format contracts, design decisions, and research notes.

## Development

Run the complete local gate before submitting a change:

```sh
just verify
```

After changing `web/src/`, regenerate the committed frontend artifacts with
`just vite::frontend-generate-artifacts`. The `just verify` gate includes
`just frontend-check`.

To regenerate the TUI GIF from a repository-controlled fixture, install
`asciinema`, `agg`, and `expect`, then run:

```sh
docs/recordings/record-tui-demo.sh
```

Run `volmap licenses` to print the embedded project and dependency notices.
