Type: grilling
Status: resolved
Blocked by: 04, 10

# Define the CLI and JSON contract

## Question

What stable command, option, output, and exit-status contract should the one `volmap` executable expose for database-name and `_vinf` inputs, summaries, static maps, targeted volume/sector/page inspection, TUI launch, HTML export, and web service launch? Decide which recovered flags remain familiar compatibility aliases, how plain and ANSI output behave, how stable JSON is versioned and streamed, and how partial results and diagnostics appear without making scripts scrape presentation text.

## Comments

### Initial contract exploration

- The current `main` intentionally accepts no arguments and produces no output; Phase 0 therefore creates no accidental compatibility surface.
- Pinned database-name discovery is `--databases-file` when explicitly selected, otherwise `$CUBRID_DATABASES/databases.txt`, then `./databases.txt`. The engine does not define the recovered program's additional `$CUBRID/databases/databases.txt` fallback. Direct `_vinf` entries contain signed volume IDs and paths; only validated nonnegative data-volume entries enter the snapshot.
- The resolved inspection model requires every projection to declare `SnapshotId`, inspection revision, coverage, and outcome. Adapters may format and select facts but may not derive, classify, re-severity, or make scripts parse human messages.
- The resolved outcome classes are `success`, `success-limited`, `findings`, `incomplete`, and `fatal`. The first two exit zero; the latter three exit nonzero. This ticket must assign stable integer codes without collapsing their model meanings.
- Recovered artifacts are quarantined and are not an authorized interface specification. The known recovered presentation flags must not silently become compatibility commitments without an explicit human decision.
- At this exploration point the command hierarchy, input grammar, output families, and compatibility policy were still pending; Q1-Q4 below record their later resolution.
- Human decision Q1 (2026-08-19): the executable uses explicit intent-based commands: `summary`, `map`, `inspect`, `tui`, `export html`, `serve`, and `licenses`. `inspect` owns targeted entities and `map` owns static allocation maps. Invoking `volmap` without a command prints concise help and performs no scan.
- Human decision Q2 (2026-08-19): every snapshot-reading command requires exactly one named input, `--database NAME` or `--vinf PATH`. Database-name lookup uses an explicit `--databases-file FILE` first, otherwise `$CUBRID_DATABASES/databases.txt`, then `./databases.txt`; there is no recovered `$CUBRID/databases` fallback. An explicit `--volume-root DIR` may accompany `--vinf` to remap copied data-volume entries by validated basename without silently searching elsewhere.
- Human decision Q3 (2026-08-19): finite commands accept `--format human|json|jsonl` and default to `human`. Terminal detection affects only `--color auto|always|never` in the human renderer; it never changes selected fields or implicitly chooses JSON. `tui`, `export html`, and `serve` have fixed output behavior and reject `--format`.
- Human decision Q4 (2026-08-19): version one preserves no recovered flags as compatibility aliases, including `--plain`, `--rows`, and `--no-overlay`. The redesigned CLI is documented on its own terms, rejects unknown flags clearly, and makes no recovered-interface compatibility claim.
- Human decision Q5 (2026-08-19): `summary` performs fast inspection and projects the overview; `map` projects fast allocation topology without deep decoding; `inspect SELECTOR` performs only enrichment required by its target; `tui` starts after fast inspection; `export html` freezes one revision; `serve` launches the web adapter; and `licenses`, global `--help`, and global `--version` require no snapshot. Dedicated tickets retain ownership of detailed TUI, export, and server options.
- Human decision Q6 (2026-08-19): all commands use one typed ASCII selector grammar: `volume:VOLID`, `sector:VOLID:SECTID`, `file:VOLID:FILEID`, `page:VOLID:PAGEID`, `slot:VOLID:PAGEID:SLOTID`, and `oos:VOLID:PAGEID:SLOTID`. Components are canonical nonnegative decimal integers with no guessing, hexadecimal, ranges, paths, or omitted volume IDs. Selectors are adapter addresses, not canonical entity references.
- Human decision Q7 (2026-08-19): `--volume-root` is valid only with `--vinf` and maps each validated data-volume entry to `ROOT/basename(recorded-path)`. Volmap rejects invalid/duplicate basenames, canonicalizes candidates, requires regular files contained under the canonical root, and still validates header identities. Without remapping it uses recorded paths exactly; it never searches, accepts snapshot input on stdin, or infers individual replacements.
- Human decision Q8 (2026-08-19): snapshot commands may expose `--tde-keys-file`, resident/spill/worker/chain-step/decoded-byte resource controls, an optional safe spill directory, and `--progress auto|always|never`. The CLI exposes no decoder/profile, cache, index, raw-byte, repair, or write controls.
- Human decision Q9 (2026-08-19): stdout contains only the requested human, JSON, or JSONL result. Usage errors, failures before a graph can be represented, and optional progress use stderr. Canonical diagnostics live inside the result and are never duplicated as ad-hoc stderr text. Machine formats default progress to `never`.
- Human decision Q10 (2026-08-19): plain human output is UTF-8 with ASCII structural characters and no escapes, cursor control, hyperlinks, or automatic pager. ANSI output may add only SGR styling and cannot change facts, order, or abbreviation. `--color auto` requires a TTY and honors `NO_COLOR`; `always` is explicit. Human reports lead with outcome, snapshot/revision, and coverage.
- Human decision Q11 (2026-08-19): JSON uses schema name `volmap.inspection` and independent integer `schema_version: 1`. Every document contains command metadata, snapshot identity/revision/validity/profile, outcome, coverage, typed data, and canonical diagnostics. Entity references are typed objects, enums use lowercase kebab-case, arrays use canonical order, and potentially 64-bit integers are decimal strings. Additive fields are compatible; removal or semantic change requires a new schema version.
- Human decision Q12 (2026-08-19): JSONL is UTF-8 with one object per LF-terminated line: header, canonically ordered typed records, then completion. Every line declares schema version, record type, sequence, snapshot ID, and pinned revision. Diagnostics occur once with backlinks. A missing completion record means truncation; an interrupted stream emits a final incomplete completion record when possible and exits nonzero.
- Human decision Q13 (2026-08-19): stable exit statuses are `0` for `success` or `success-limited`, `1` for `findings`, `2` for invalid invocation/option/selector/cursor, `3` for `incomplete`, `4` for `fatal`, `70` for an internal inspector defect, and `128 + signal` for caught signal termination. JSON/JSONL retains the full outcome and diagnostic identity; exit status is deliberately coarse.
- Human decision Q14 (2026-08-19): JSON and JSONL contain every canonical diagnostic and coverage ledger contributing to the command outcome. Human output always shows global severity/code counts and every error/fatal occurrence; warning/info detail may be grouped unless `--diagnostics full` is requested. Partial coverage names its facet, stop reason, evaluated count, trusted total when known, and suppressed or explicitly unknown remainder.
- Human decision Q15 (2026-08-19): the one-shot CLI exposes no offsets or pagination cursors because each process creates a new `SnapshotId`. Commands emit the complete selected projection in canonical order; users narrow maps with a typed entity selector and use JSONL for large streams. Version one has no generic `--offset`, `--limit`, filter expression, or sort override.
- Human decision Q16 (2026-08-19): version one reads no Volmap configuration file. Only `CUBRID_DATABASES`, `NO_COLOR`, and the platform temporary-directory environment may provide established defaults; explicit CLI options take precedence. Duplicate scalar options are errors. TDE key paths and other security-sensitive values are never accepted from the environment.
- Human decision Q17 (2026-08-19): machine formats accept `--schema-version 1` and default to version 1 while it is the sole version. Unsupported versions fail before scanning with exit 2. Future binaries may support multiple versions but must continue producing an explicitly requested version while it remains supported; automation should pin one.
- Human decision Q18 (2026-08-19): unavailable and optional JSON facts use discriminated objects plus canonical availability/coverage states rather than implicit `null`, zero, or empty sentinels. Unknown, unreadable, unsupported, opaque, unresolved, and known absence remain distinct. `null` appears only where the schema explicitly defines known absence.
- Human decision Q19 (2026-08-19): JSON, JSONL, canonical diagnostics, and human reports omit `_vinf`, volume-root, spill, and TDE-key paths, nonces, key identifiers/hashes, ciphertext, and application data. A pre-graph local stderr usage/I/O error may quote the exact user-supplied path only after control escaping.
- Human decision Q20 (2026-08-19): caught `SIGINT`/`SIGTERM` attempts to finish JSONL with an incomplete completion record and exits 130/143. `EPIPE` terminates quietly with 141 and no stderr noise. Other output-write failures exit 4; consumers treat an invalid JSON document or JSONL stream without completion as truncated.
- Human decision Q21 (2026-08-19): command names, option meanings, selector grammar, exit codes, JSON/JSONL schemas, enum meanings, and canonical ordering are stable version-one contracts. Human wording, spacing, widths, SGR styling, progress text, and stderr prose are non-machine interfaces and may evolve compatibly.
- Human decision Q22 (2026-08-19): resource counts accept unsigned ASCII decimal integers; byte quantities additionally accept exact IEC suffixes `KiB`, `MiB`, `GiB`, and `TiB`. Signs, fractions, locale separators, unknown suffixes, overflow, and semantically invalid zero values are rejected before snapshot files are opened.
- Human decision Q23 (2026-08-19): `map` without a selector covers the full snapshot and accepts only `volume`, `sector`, or `file` selectors. A file selector projects its potentially discontiguous allocation. Page, slot, and OOS map selectors are request errors with exit 2 and belong to `inspect` instead.
- Human decision Q24 (2026-08-19): global and command-specific `--help` plus global `--version` write to stdout and exit 0 without scanning. Bare `volmap` writes concise help to stderr and exits 2 because the required intent is missing.
- Human decision Q25 (2026-08-19): a syntactically valid selector whose entity does not exist in the validated snapshot is a request error, not corruption, and exits 2. Human mode writes a control-escaped explanation to stderr. JSON/JSONL emits a versioned command-error object with stable code `entity-not-found` and the safe selector, but creates neither an entity nor a diagnostic occurrence.

## Answer

The standalone `volmap` executable uses explicit commands, explicit snapshot inputs, typed entity selectors, and an explicit output family. Human text is never a machine interface; stable automation uses versioned JSON/JSONL fields, diagnostic codes, coverage ledgers, canonical ordering, and documented exit statuses.

### Command grammar

```text
volmap summary INPUT [SCAN_OPTIONS] [OUTPUT_OPTIONS]
volmap map INPUT [SELECTOR] [SCAN_OPTIONS] [OUTPUT_OPTIONS]
volmap inspect INPUT SELECTOR [SCAN_OPTIONS] [OUTPUT_OPTIONS]
volmap tui INPUT [SCAN_OPTIONS]
volmap export html INPUT --output PATH [SCAN_OPTIONS] [...]
volmap serve INPUT [SCAN_OPTIONS] [...]
volmap licenses [...]
volmap --help | --version
```

`INPUT` is exactly one of:

```text
--database NAME [--databases-file FILE]
--vinf PATH [--volume-root DIR]
```

`--database` lookup uses the explicit `--databases-file` when present, otherwise `$CUBRID_DATABASES/databases.txt`, then `./databases.txt`. There is no `$CUBRID/databases` fallback. `--databases-file` is invalid with `--vinf`; `--volume-root` is invalid with `--database`. Snapshot input is never read from stdin.

For a direct `_vinf`, paths are used exactly as recorded unless `--volume-root` is explicit. Remapping takes only each validated nonnegative data-volume entry's basename, rejects empty/invalid/duplicate basenames, joins it to the canonical root, and requires the canonical regular-file target to remain under that root. Header identity and volume-chain checks remain mandatory; remapping never searches for alternatives or repairs the manifest.

Bare `volmap` prints concise help to stderr and exits 2 without scanning. Global or command-specific `--help` and global `--version` print to stdout and exit 0 without scanning. Unknown commands/options, invalid combinations, duplicate scalar options, invalid numeric values, and irrelevant options fail before opening snapshot files.

### Command behavior

- `summary` performs the complete fast inspection and emits the canonical overview, outcome, coverage, summary counts, and diagnostics.
- `map` performs fast inspection only and emits allocation topology for the entire snapshot or one optional `volume`, `sector`, or `file` selector. A file map may be physically discontiguous. Page, slot, and OOS map selectors are request errors.
- `inspect` accepts exactly one selector. Volume, sector, and file targets project fast facts; page targets request that page's supported deep detail; slot targets first validate/decode the containing page and then select the slot; OOS targets traverse only that selected chain under its budgets. It never auto-traverses heap OOS references or emits forbidden payload.
- `tui` launches its adapter only after the initial fast revision is published. Its navigation/interaction contract belongs to “Prototype the TUI navigation and inspection flow.”
- `export html` freezes one revision after any explicitly supported enrichment. File/export behavior and included viewer data belong to “Define the web service, remote-access, and HTML-export contract” and “Prototype the web sector and slotted-page explorer.”
- `serve` launches the web adapter over the same inspection module. Listener/authentication/export flags remain owned by “Define the web service, remote-access, and HTML-export contract.”
- `licenses` requires no snapshot and accepts the same explicit output-family selector as other finite commands. Its notice contents and separate machine notice schema remain owned by “Evaluate the final Rust dependency and reproducible-release graph”; this ticket fixes the command and output-family behavior, not that later payload.

The exact accepted scan options are:

```text
--tde-keys-file PATH
--memory-limit BYTES
--spill-limit BYTES
--workers COUNT
--max-chain-steps COUNT
--max-decoded-bytes BYTES
--spill-directory DIR
--progress auto|always|never
```

Options that cannot affect a command are rejected rather than silently ignored. `--max-chain-steps` and `--max-decoded-bytes`, for example, apply only to commands/adapters capable of requested deep traversal. Numeric defaults are supplied by “Measure representative scan performance and set resource-budget defaults.” Counts are unsigned ASCII decimal; byte values additionally accept exact case-sensitive IEC suffixes `KiB`, `MiB`, `GiB`, and `TiB`. Signs, fractions, separators, unknown suffixes, overflow, and semantically invalid zero are usage errors.

There is no Volmap configuration file. The only recognized environmental defaults are `CUBRID_DATABASES`, `NO_COLOR`, and the platform temporary-directory environment. Explicit applicable CLI options win. TDE key paths and other sensitive values never come from an environment variable. Version one exposes no format/decoder selection, cache/index tuning, raw-byte escape hatch, repair, write, or recovered compatibility option.

### Entity selector

The canonical CLI spelling is one ASCII token:

```text
volume:VOLID
sector:VOLID:SECTID
file:VOLID:FILEID
page:VOLID:PAGEID
slot:VOLID:PAGEID:SLOTID
oos:VOLID:PAGEID:SLOTID
```

Every component is canonical nonnegative decimal with no sign, alternate radix, leading/trailing whitespace, range, path, or omitted volume. `slot` and `oos` deliberately distinguish two entity kinds that share an OID-shaped physical key. A selector addresses the newly inspected snapshot for one adapter request; it is not stored as a canonical entity reference.

Malformed or inapplicable selectors fail before scanning when possible and exit 2. A well-formed selector that is conclusively absent from the validated snapshot is also a request error, not an on-disk diagnostic. Human mode explains it on stderr. JSON uses a `volmap.inspection` versioned `command-error` document with code `entity-not-found`, the safe selector, and available snapshot identity/revision; JSONL emits header, command-error, and completion records. If corruption or unreadability prevents proving presence/absence, the command instead returns the canonical findings/incomplete result and its evidence—it never mislabels uncertainty as not found.

### Output selection and channels

Snapshot-producing finite commands accept:

```text
--format human|json|jsonl       # default: human
--color auto|always|never       # human only
--diagnostics summary|full      # human only; default: summary
--schema-version 1              # json/jsonl only
```

`tui`, `export html`, and `serve` have fixed adapter output and reject `--format`. `licenses` accepts the explicit family, but its machine records use the separate later-owned notice schema rather than `volmap.inspection`. Unsupported schema versions fail before scanning with exit 2. Version 1 is the default while it is the sole supported schema, but automation should request it explicitly.

Stdout contains only the requested result. Stderr contains usage errors, safely escaped pre-graph operational errors, and optional progress. Canonical diagnostics are part of the result and are never duplicated into ad-hoc stderr prose. Machine formats default `--progress never`; human output uses `auto`. Progress never carries paths/bytes and is not canonical evidence.

Plain human output is UTF-8 with ASCII structural characters and no escape sequences, cursor control, hyperlinks, or automatic pager. ANSI mode may add SGR styling only; it cannot add, omit, reorder, or abbreviate facts. `auto` styles only a TTY and honors `NO_COLOR`; `always` is an explicit override. Every human report begins with inspection outcome, snapshot/revision/validity, and coverage before summaries and diagnostics.

Human output always displays global diagnostic counts by severity and code plus every error/fatal occurrence. Warning/info detail may be grouped in `summary` mode; `--diagnostics full` renders every occurrence. Partial coverage always names the facet, stop boundary/reason, evaluated count, a total only when independently trusted, and a known suppressed count or explicit unknown remainder. Human wording/layout is not intended for scripts.

### JSON document contract

Every inspection result is one UTF-8 JSON object with this stable envelope:

```text
schema              "volmap.inspection"
schema_version      1
document_type       "result" | "command-error"
tool                name and build/version identity
command             command, safe selector if any, and input kind (never paths)
snapshot            SnapshotId, revision, validity, and format profile when established
outcome             canonical inspection outcome for result documents
coverage            canonical coverage summaries and ledgers
data                typed command projection
diagnostics         canonical diagnostic occurrences
error               stable request-error object for command-error documents
```

The exact entity, relationship, evidence, diagnostic, containment, and coverage objects project the canonical graph definitions from “Define the canonical inspection model and evidence levels” and “Define corruption containment and diagnostic semantics”; adapters do not create parallel meanings. Entity references are typed objects, not selector strings or URLs. Snapshot IDs are opaque lowercase hexadecimal strings. Potentially 64-bit revisions, evidence/diagnostic identifiers, byte offsets, lengths, counts, and sequence values are decimal strings; bounded physical identifiers may remain JSON integers. Enums are lowercase kebab-case. Arrays use canonical model ordering.

JSON always includes every diagnostic occurrence and coverage ledger contributing to the command outcome, so a nonzero exit never requires parsing stderr or message prose. Raw payload, ciphertext, decrypted values, nonces, key identifiers/hashes, key material, and `_vinf`, volume-root, spill, TDE-key, or other nonessential host paths are absent. The command envelope reports only input kind and a safe selector. User-supplied paths may appear only in control-escaped local stderr when no graph result can exist.

Unavailable/optional facts use discriminated objects and canonical availability/coverage values. Unknown, unreadable, unsupported, encrypted-opaque, unresolved, and known absence are never collapsed into `null`, zero, empty text, or an empty collection. `null` is permitted only where schema version 1 explicitly defines semantic known absence. Consumers must ignore unknown additive object fields and unknown record types they do not require; removing a field, changing a field/enum meaning, or reusing a diagnostic meaning requires a new schema version.

### JSONL stream contract

JSONL is the complete selected projection, not pagination. It emits one compact JSON object per LF-terminated line:

1. one `header` record;
2. canonically ordered typed entity/fact/relationship/evidence/diagnostic/coverage records;
3. one `completion` record carrying final outcome, coverage/count summaries, and process-status class.

Every line repeats `schema`, `schema_version`, `record_type`, monotonic decimal-string `sequence`, `snapshot_id`, and pinned decimal-string `revision` when available. Each diagnostic occurrence is emitted once; entities/relationships/ledgers refer to its occurrence ID. One command pins one revision for its whole projection, even if an interactive inspection session later advances.

A completion record is the only proof that a stream is whole. Interruption attempts to emit an incomplete completion record before returning nonzero. A missing completion record, malformed line, sequence gap, snapshot/revision change, or output-write failure means the stream is truncated and must not be treated as a complete projection. The CLI exposes no cross-process cursor, offset, generic limit, filter expression, or sort override: every process creates a new `SnapshotId`, while JSONL already provides bounded-memory streaming.

### Exit-status contract

```text
0    success or success-limited
1    findings
2    invalid/missing command, option, combination, value, schema, or selector;
     also a conclusively absent selected entity
3    incomplete
4    fatal inspection/root/configuration/output failure
70   internal inspector defect (`inspection.internal_error`)
130  caught SIGINT
143  caught SIGTERM
141  broken stdout pipe (`EPIPE`), without stderr noise
```

Other caught signals use `128 + signal`. A signal-driven inspection still retains the canonical `incomplete` outcome when it can emit a result; the signal-specific process code preserves shell convention. Other output-write failures exit 4. JSON may therefore be syntactically incomplete and JSONL may lack completion; exit status and framing both indicate failure. Exit status deliberately compresses the richer model and never replaces outcome, availability, coverage, severity, or diagnostic code.

### Stability boundary

Within schema/CLI version one, command names, option names and meanings, valid combinations, selector grammar, exit statuses, JSON/JSONL fields and enum meanings, diagnostic meanings, numeric encodings, and canonical ordering are compatibility commitments. Additive JSON fields/record types are compatible. Human wording, whitespace, table widths, grouping, SGR choices, progress text, help prose, and stderr prose may improve without a schema change and must never be scraped by automation.
