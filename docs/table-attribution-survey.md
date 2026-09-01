# Table attribution survey

> **Implementation status (2026-09-01).** This is the source-pinned design
> survey that preceded the production implementation. The released Page
> projection and its fail-closed states are documented in the
> [Page file/class association contract](page-association-contract.md). The
> historical “missing work” statements below describe the survey baseline, not
> the current code. Production attribution is per Page; sector attribution is a
> separate claim summary, and OOS class attribution remains deferred.

## Scope and source baseline

This survey asked how the offline volume inspector could show a CUBRID
class/table name for a Page and explored a possible sector summary. OOS was
intentionally deferred; the scoped file families were heap, heap-reuse,
multipage heap overflow, B-tree, B-tree overflow-key, extensible-hash
bucket/directory, and catalog.

The volmap worktree was inspected on `main` at `7e58c4e496cb68ae585e14b227e5cda9b02ad153`. Its README pins the interpreted CUBRID format to `e1e651debf6cc100172bde96603b17424f9c135a`, a commit contained by `feat/oos`; all CUBRID layout claims below are against that exact commit. The nearby non-OOS `develop` worktree is at `f30f1c26003e5aa8e93182648e06cad76fc77064`; its relevant pre-OOS descriptor definitions are unchanged. See the [volmap format pin](/home/vimkim/temp/volmap/README.md:5) and CUBRID's stable [`FILE_DESCRIPTORS` definitions](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/file_manager.h#L81-L149).

## Conclusion

The requested feature is feasible fully offline. The reliable path is:

```text
physical page (VPID)
  -> file allocation table claim (VFID)
  -> file header descriptor (class_oid)
  -> class object record at that exact OID
  -> first variable attribute (stored class/table name)
```

The name is **not** embedded in an arbitrary heap, overflow, B-tree, or hash page. Nor should volmap search the volume for a matching string. The owning/associated file header physically stores `class_oid`, and that OID directly addresses the class record whose first variable attribute is the name. This is also the path used by the engine: `file_header_dump_descriptor` calls `heap_get_class_name`, which reads the class record and calls `or_class_name` ([file dump](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/file_manager.c#L1423-L1469), [name lookup](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/heap_file.c#L9661-L9745), [record decoding](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/object/object_representation.c#L220-L271)).

Volmap already has the first half of the graph. `SessionData` retains `file_allocations: VPID -> VFID` and `tracked_files: VFID -> FileHeader`, but `PageView`, `SectorView`, and their projections do not expose those relationships ([inspection state](/home/vimkim/temp/volmap/src/inspection.rs:905), [page and sector views](/home/vimkim/temp/volmap/src/inspection.rs:212), [projections](/home/vimkim/temp/volmap/src/projection.rs:149)). The missing work is therefore modest descriptor coverage plus a bounded class-record/name resolver and projections—not a new page scan architecture.

## What is physically available

The CUBRID file header puts a fixed 64-byte type-specific descriptor at user-data offset 40. Its ABI is explicitly disk-compatible. The scoped mappings are:

| File type | Descriptor evidence | Class OID offset from descriptor start | Attribution meaning |
|---|---|---:|---|
| `FILE_HEAP`, `FILE_HEAP_REUSE_SLOTS` | `FILE_HEAP_DES { class_oid, hfid }` | `+0` | Owning class/table |
| `FILE_MULTIPAGE_OBJECT_HEAP` | `FILE_OVF_HEAP_DES { hfid, class_oid }` | `+12` | Owning heap's class/table |
| `FILE_BTREE` | `FILE_BTREE_DES { class_oid, attr_id }` | `+0` | Top/associated class for the index |
| `FILE_BTREE_OVERFLOW_KEY` | `FILE_OVF_BTREE_DES { btid, class_oid }` | `+12` | Same class association as its B-tree |
| `FILE_EXTENDIBLE_HASH`, directory | `FILE_EHASH_DES { class_oid, attr_id }` | `+0` | Associated class when non-null; often internal metadata rather than a user table |
| `FILE_CATALOG` | no class descriptor | n/a | One global catalog file; no single table owner |

The offsets follow the fixed C layouts in [`file_manager.h`](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/file_manager.h#L81-L149) and the descriptor's placement in [`FILE_HEADER`](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/file_manager.c#L85-L123). Engine code writes the values when it creates each file:

- Heap creation writes the class OID to both the file descriptor and heap header slot 0; multipage overflow creation copies the heap header's class OID into its descriptor ([heap](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/heap_file.c#L4871-L4899), [heap overflow](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/heap_file.c#L6117-L6134)).
- B-tree creation stores `class_oid` and `attr_id`; overflow-key creation stores the parent BTID and `topclass_oid` ([B-tree](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/btree.c#L34573-L34613), [overflow key](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/btree.c#L1985-L2008)).
- Extensible-hash creation copies the same descriptor into its bucket and directory files. The API permits a null class OID, so not every hash is a user-table association ([creation contract](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/extendible_hash.c#L832-L869), [descriptor population](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/extendible_hash.c#L953-L990), [both files](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/extendible_hash.c#L1036-L1087)).

As a semantic cross-check, `file_tracker_interruptable_iterate` reopens class-owned file headers and selects these same class-OID members for heap, multipage heap overflow, B-tree, and B-tree overflow-key files ([engine selection](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/file_manager.c#L10929-L11002)).

Volmap currently decodes the descriptor OID only for heap and heap-reuse files. Multipage overflow retains only its HFID, and the B-tree, B-tree-overflow, and hash OIDs are ignored ([current decoder](/home/vimkim/temp/volmap/src/format/file_table.rs:222)). The heap page decoder independently obtains a class OID from heap header/chain slot 0, and the B-tree root decoder obtains `top_class`, but those facts are not present on every data/child page ([heap decoder](/home/vimkim/temp/volmap/src/format/heap.rs:289), [B-tree root decoder](/home/vimkim/temp/volmap/src/format/btree.rs:57)). File attribution is the common path that covers every allocated page in all scoped families.

## Offline `class_oid -> name` resolution

`class_oid` is already the physical `(volid, pageid, slotid)` address of the class object; it is not a key that requires scanning the catalog. `heap_get_class_record` passes that OID to the ordinary heap last-version reader with the root class as the object's class ([engine record fetch](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/heap_file.c#L27110-L27142)). MVCC is disabled for the root class, simplifying offline class-record reading ([MVCC rule](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/transaction/mvcc.c#L619-L653)).

A fail-closed resolver should:

1. Collect each distinct non-null descriptor `class_oid` after the file inventory has been validated.
2. Read exactly that volume page and slot. Reuse volmap's bounded relocation and multipage-heap `REC_BIGONE` traversal when the class object is indirect; never scan unrelated payload looking for text.
3. Validate the object-representation header and variable-offset width, then obtain variable attribute 0 exactly as `OR_VAR_OFFSET(record, 0)` does.
4. Decode the packed VARCHAR prefix: one byte when the first byte is not `0xff`; otherwise read both big-endian 32-bit lengths (compressed and decompressed) before the payload. An uncompressed long value uses a zero compressed length; a non-zero compressed length requires LZ4 decoding. Require all bounds and the terminating NUL instead of relying on the engine helper's unchecked pointer return.
5. Convert the name bytes from the database codeset to UTF-8 for JSON/web output, enforce CUBRID's identifier bound, and cache the result by OID.
6. On a missing page, bad slot, unsupported record/encoding, encrypted-opaque page, corrupt offsets, or dangling/dropped class, preserve the OID and return a typed `unresolved` reason. Never manufacture a name.

The database codeset is not an external prerequisite. CUBRID stores it in every volume header ([disk header](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/disk_manager.c#L73-L97)); volmap already decodes `database_charset` at byte 30 but currently drops it when constructing `VolumeRecord` ([decoder](/home/vimkim/temp/volmap/src/format/volume.rs:54), [discard point](/home/vimkim/temp/volmap/src/inspection.rs:1188)). Preserve the value, require consistency across volumes, and support CUBRID's ASCII, ISO-8859-1, EUC-KR, and UTF-8 codesets rather than assuming UTF-8 ([codeset enum](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/base/intl_support.h#L177-L190)). `DB_MAX_IDENTIFIER_LENGTH` permits 255 identifier bytes; CUBRID buffers commonly reserve one additional byte for the terminator ([identifier bound](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/compat/dbtype_def.h#L525-L530)).

The class-name extensible hash recorded in `BOOT_DB_PARM` maps names to OIDs and is therefore the wrong direction for this task. It would add boot-record and hash traversal without improving correctness. A running CUBRID instance can resolve the same names through its dump routines, but `diagdb` restarts the database before dumping ([utility startup](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/executables/util_sa.c#L1586-L1624)); making volmap depend on that would discard its offline, no-engine property. It remains useful only as a validation oracle on a copied test database.

## Page attribution

For an allocated page, the defensible relationship is:

```text
Page --allocated by--> File --associated with--> Class --named--> stored class name
```

This deliberately separates physical page type, file role, and class association. Examples are `dba.orders · heap`, `dba.orders · btree`, and `dba.orders · heap overflow`. A B-tree or hash has an associated/top class, which is not necessarily semantically identical to saying that every indexed OID belongs only to one SQL table. A null or system class association should remain visible as internal/unresolved rather than being labeled as a user table.

Volmap's inventory already discovers all tracker entries, decodes their headers, and reconstructs allocated pages from each file's partial/full sector tables ([inventory](/home/vimkim/temp/volmap/src/inspection.rs:2218), [allocation collection](/home/vimkim/temp/volmap/src/inspection.rs:2347)). It currently rejects a duplicate allocated-page claim with `file.table.owner_unique`, which is appropriately fail-closed ([duplicate check](/home/vimkim/temp/volmap/src/inspection.rs:2270)). Therefore no page-family-specific parent walk is necessary for normal attribution.

Do not copy the table string into the compact fact for every physical page. Retain normalized shared maps—`VPID -> VFID`, `VFID -> descriptor/class_oid`, and `class_oid -> resolution`—and join them when producing `PageView`/`PageProjection`. This preserves the fast-scan memory contract and naturally makes a rename visible from one cached resolution.

## Sector attribution and mixed behavior

CUBRID reserves disk space by sector and allocates file pages within those reservations. A partial-sector file-table entry is `(VSID, page_bitmap)`; set bits are allocated pages, while unset bits are still pages reserved for that file. A full-sector entry means all 64 pages are allocated ([descriptor structures](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/file_manager.h#L161-L177), [engine mapping](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/file_manager.c#L7312-L7373), [partial dump terminology](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/file_manager.c#L7748-L7803)). Thus:

- An allocated page can be attributed to its file/class.
- An unset page in a file's partial sector is **reserved for** that file/class, not yet owned as an allocated file page.
- An unreserved page has no file/class attribution.
- A system sector or catalog sector should be labeled internal/system, not as a user table.

Current `collect_file_allocations` creates a local sector set, validates uniqueness only within one file, expands bitmap-set/full-sector pages, and returns only the page set. It discards the sector claim and partial bitmap ([current loss point](/home/vimkim/temp/volmap/src/inspection.rs:2426)). Deriving a sector label from the returned allocated pages would consequently miss a completely empty reserved sector and would misdescribe unset pages.

Preserve each per-file sector claim and project sector attribution as a tagged state:

| State | Display rule |
|---|---|
| `single` | One valid file claim: show resolved table/class plus file role and `allocated/64`; unset partial bits are `reserved-for` |
| `mixed` | Multiple file or incompatible claims: show `mixed`, retain every claim, and emit a diagnostic; never select a majority owner |
| `internal` | Tracker/catalog/volume metadata or another non-class file: show the file/system role |
| `unowned` | No file reservation claim |
| `unresolved` | A file/class OID exists but its descriptor or class name cannot be validated; show identifiers and reason |

Under normal CUBRID invariants, one reserved sector belongs to one file. `mixed` is still necessary for corrupt or inconsistent snapshots. Two bad file tables could claim disjoint page bitmaps in the same sector; the current page-collision check would not detect that. Even if multiple claims resolve to the same table, retain `mixed` at the file-claim layer because the physical inconsistency remains.

## Catalog is not table-owned

`FILE_CATALOG` is one global file with no class descriptor, paired with an internal extensible hash ([catalog identifier](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/system_catalog.h#L42-L50), [catalog creation](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/system_catalog.c#L2613-L2667)). Catalog records for different classes are inserted into any global catalog page with suitable free space ([page selection](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/system_catalog.c#L679-L725), [class-info insertion](https://github.com/CUBRID/CUBRID/blob/e1e651debf6cc100172bde96603b17424f9c135a/src/storage/system_catalog.c#L3019-L3125)).

Therefore a catalog page or sector must not be assigned one table name. Volmap may later resolve and show the class OIDs already exposed on individual catalog directory facts, but that is record-level content annotation, not page ownership.

## Recommended minimal change

1. **Complete descriptor decoding.** Replace the heap-only `class_oid` special case with a typed descriptor fact, or at minimum add `owner_class_oid()` for all scoped file types while retaining HFID/BTID/attribute relationships. Validate the exact offsets above with source-derived fixtures.
2. **Retain normalized associations.** Add a shared file-association table and `BTreeMap<Oid, ClassNameResolution>`, where resolution is tagged `resolved { name }` or `unresolved { reason }`. Preserve the numeric OID in both states.
3. **Add the bounded offline resolver.** Resolve distinct OIDs once, using the existing heap slot, relocation, and multipage-overflow readers. Retain and validate the database codeset from volume headers. Apply normal inspection resource budgets and cancellation.
4. **Retain sector claims.** Return partial/full `SectorClaim { vfid, kind, bitmap }` facts alongside allocated pages. Validate uniqueness across all files and derive the tagged sector state described above.
5. **Project, do not reparse.** Add a tagged `file_association`/`class_association` field to `PageView`, `SectorView`, and the stable projections. All CLI/TUI/web adapters should consume that shared fact. The current JSON schema is explicitly versioned at 1 ([projection schema](/home/vimkim/temp/volmap/src/projection.rs:14)); make the fields additive if that is the compatibility policy, otherwise increment the schema version deliberately and update contract fixtures.
6. **Keep the web design.** Add `Class/table`, `Class OID`, `File`, and `File role` rows to the existing Page facts panel. Add one ellipsized class/table label to a sector card and sector title only for `single`; use a tooltip/accessible label for the full name, and show `mixed`, `internal`, or `unknown` for the other states. The existing page facts and sector-card construction are localized in [web rendering](/home/vimkim/temp/volmap/src/web.rs:1683).

A compact projection shape could be:

```json
{
  "file_association": {
    "state": "allocated",
    "vfid": { "vol_id": 1, "file_id": 640 },
    "file_type": "heap-reuse-slots",
    "class_oid": { "vol_id": 0, "page_id": 195, "slot_id": 2 },
    "class_name": { "state": "resolved", "value": "dba.fixture_rows" }
  }
}
```

For a partial sector, the analogous `single` object should include `allocated_pages` and `reserved_unallocated_pages`. These are different facts and should not be collapsed into one owner string.

## Options and trade-offs

| Option | Offline | Coverage | Trade-off |
|---|---:|---|---|
| Direct descriptor + class-record OID read | Yes | All scoped class-associated files; allocated pages and retained sector reservations | Recommended. Smallest trustworthy dependency surface; requires a narrow object-record/name decoder and codeset conversion |
| Scan root-class heap and build all OID/name pairs | Yes | All classes, including those with no tracked file | More I/O and parser surface than needed; useful only for a future class browser |
| Reverse/read the classname E-hash | Yes | Name-to-OID index | Wrong direction and additional boot/hash complexity; not recommended |
| Sidecar produced by SQL/engine dump | Snapshot-dependent | Whatever was exported | Easy validation route, but stale/mismatched metadata is possible and standalone/offline operation is lost |
| Invoke CUBRID engine/`diagdb` | No, engine restart required | Engine-supported dumps | Excellent validation oracle, poor product dependency; may perform recovery/startup work |

## Limitations and validation

Attribution is only as trustworthy as the stable stopped/snapshotted volume set and the validated tracker/file tables. A crash-inconsistent image, in-flight DDL, a dropped/reused class OID, a missing class-record page, encrypted/unreadable metadata, or an unsupported codeset must produce `unresolved`, not a guessed string. The stored class name is the engine's class name; the UI should not infer an SQL schema or strip qualification.

Recommended tests:

- Unit fixtures for every descriptor type and offset, including null/system class OIDs and malformed descriptors.
- A stopped real database with two tables, heap data, a `REC_BIGONE` multipage overflow file, multiple B-trees, B-tree overflow keys, and internal E-hashes. Compare volmap names/OIDs with `diagdb` file descriptions and SQL catalog results from the same copied snapshot.
- Rename a table and verify that the stable class OID resolves to the new stored name without changing page/file attribution.
- Exercise class records that are home, relocated, and `REC_BIGONE`; corrupt the target slot, variable offsets, VARCHAR length, and terminator independently and assert typed unresolved results.
- Test UTF-8, EUC-KR, and ISO-8859-1 database identifiers using the volume-header codeset.
- Test a partial sector with no allocated user pages, a full sector, and two forged files claiming disjoint bitmaps in one sector. The outputs should be `single/reserved-for`, `single/full`, and `mixed` respectively.
- Put catalog records for multiple classes on one page and assert that the page remains `internal/catalog`, while record-level class references may be annotated separately.
- Verify the compact physical page-fact size and memory benchmark do not grow by one string per page.

The pinned fixture already supplies representative heap, B-tree, and multipage-overflow file headers, but its referenced fixture class OID is `vol 0 / page 195 / slot 2` and page 195 is not included in the current page corpus ([fixture manifest](/home/vimkim/temp/volmap/fixtures/e1e651de/manifest.toml:98), [corpus page list](/home/vimkim/temp/volmap/tests/pinned_fixture_corpus.rs:22)). A class-record page plus its necessary heap/overflow context should be added before claiming an end-to-end offline-name fixture.

## Deferred OOS note

OOS attribution is outside this proposal at the user's request. Re-evaluate it against the target `feat/oos` baseline when that work starts; do not let the non-OOS implementation infer OOS ownership from HFID alone.
