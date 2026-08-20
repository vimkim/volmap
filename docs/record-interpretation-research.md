# Heap record interpretation research

## Scope and source baseline

This note answers what a standalone offline reader must do to turn raw CUBRID volume bytes into interpreted attribute values for a normal in-row heap record, and whether per-sector caching of class representations is sound.

The primary source is the CUBRID worktree at `/home/vimkim/gh/cb/feat-oos`, branch `feat/oos`, HEAD `465cf53e3`. Every layout, offset, and behavior claim below cites `path:line` relative to that worktree root plus the enclosing function or struct. Where a claim needed a non-OOS comparison, the citation says so and the compared tree is `/home/vimkim/gh/cb/develop` at `e6ed61e87`. Nothing here is from memory or from web write-ups.

Structural constants that this document asserts as byte offsets were cross-checked two ways wherever possible: against the C struct declaration, and against volmap's already-working decoders, which read real demodb volumes. Those cross-checks are called out inline. Claims that rest only on a derived (compiled) struct layout are flagged in the final section.

## TL;DR

- **A heap record is four regions in order:** a variable-size MVCC header (8/16/24/32 bytes, size determined by three flag bits packed into the record's first 4-byte word), a variable-offset table, the fixed-width attribute area, and the bound-bit array — then the variable values. The reprid is the low 24 bits of that first word (`OR_GET_MVCC_REPID`, `src/base/object_representation.h:571`).
- **CHAR and NUMERIC are *not* fixed-width on disk.** `variable_p` in the `PR_TYPE` table decides fixed vs variable, and it is 1 for `DB_TYPE_CHAR` (`src/object/object_primitive.c:12733`) and `DB_TYPE_NUMERIC` (`src/object/object_primitive.c:1639`). Both live in the variable region with a length prefix. This is the single easiest thing to get wrong.
- **The class of a record is available on the page itself.** Slot 0 of every `PAGE_HEAP` page holds either `HEAP_HDR_STATS` (1160 bytes) or `HEAP_CHAIN` (40 bytes), and `class_oid` is field 0 of *both* structs by deliberate design (`src/storage/heap_file.c:201`, `:285`). No catalog walk, no file-table walk.
- **Interpretation does not go through the system catalog.** The engine decodes instances against `OR_CLASSREP`, built by `or_get_classrep` from the *class object's own heap record* (`src/base/object_representation_sr.c:3351`). The catalog's `DISK_REPR`/`DISK_ATTR` records are a parallel structure used by the query optimizer for statistics. Both are keyed by the same reprid, but a reader that wants attribute domains should read the class record, not the catalog.
- **Question 5: verified, with caveats.** A sector belongs to exactly one file for that file's entire lifetime, and a heap file belongs to exactly one class. So caching a class's representation per sector is sound. The caveats that matter are that not every page in the sector holds rows, that a dropped-but-not-yet-reused heap still names the dropped class, and that partitions are separate classes with separate heaps (which the scheme handles correctly, it just resolves to the partition).
- **feat-oos changes the variable-offset table format.** The low 2 bits of every offset-table entry are now flag bits (`OR_VAR_BIT_OOS`, `OR_VAR_BIT_LAST_ELEMENT`), and readers must mask them off. On `develop` the entry is a plain offset with no masking. Details in section 7.

---

## 1. On-disk layout of a normal heap record

### 1.1 Getting to the record bytes

The physical page at index *p* of a volume file starts at file offset `p * page_size`. The first 32 bytes are `FILEIO_PAGE_RESERVED`, a native-endian system prologue; the user page area begins at `+32`. The prologue is `struct fileio_page_reserved` (`src/storage/file_io.h:166`) inside `struct fileio_page` (`src/storage/file_io.h:186`), and the last 8 bytes of the page are a duplicate LSA watermark (`fileio_get_page_watermark_pos`, `src/storage/file_io.h:195`). volmap's decoder confirms the layout in practice: `PAGE_PREFIX_SIZE = 32`, with pageid at `+8`, volid at `+12`, `ptype` at `+14`, `pflag` at `+15`, all read little-endian (`/home/vimkim/temp/volmap/src/format/page.rs:8`, `:344`).

`prv.ptype` is genuinely persisted, not merely an in-memory hint: `pgbuf_set_page_ptype` writes it into the buffered I/O page at `src/storage/page_buffer.c:5460`. An offline classifier may therefore use it, though the structural tests in section 2 are stronger.

A heap page's user area is a slotted page. `SPAGE_HEADER` is 32 bytes at user-area offset 0 (`struct spage_header`, `src/storage/slotted_page.h:64`; volmap's validated `SLOTTED_HEADER_SIZE = 32` at `/home/vimkim/temp/volmap/src/format/slotted.rs:6`). Slot descriptors are 4 bytes each and grow *downward* from the end of the user area, so slot *n* sits at `user_area_end - 4*(n+1)`. Each descriptor is a packed bitfield: `offset_to_record:14, record_length:14, record_type:4` (`struct spage_slot`, `src/storage/slotted_page.h:86`). Heap pages are initialized `ANCHORED_DONT_REUSE_SLOTS` with `HEAP_MAX_ALIGN` (= `INT_ALIGNMENT`, 4) record alignment (`heap_create_internal`, `src/storage/heap_file.c:4882`; `heap_get_spage_type`, `src/storage/heap_file.c:1057`; `HEAP_MAX_ALIGN`, `src/storage/heap_file.h:64`).

The record type in the slot is one of the values in the anonymous enum at `src/storage/storage_common.h:1157`: `REC_HOME = 2`, `REC_NEWHOME = 3`, `REC_RELOCATION = 4`, `REC_BIGONE = 5`, `REC_MARKDELETED = 6`, `REC_DELETED_WILL_REUSE = 7`. Only `REC_HOME` and `REC_NEWHOME` slots contain an actual object image. `REC_RELOCATION` and `REC_BIGONE` contain a forwarding OID.

### 1.2 The first word: reprid, bound-bit flag, offset size, record flags

Every object image begins with one 4-byte big-endian word at offset 0 (`OR_REP_OFFSET = 0`, `src/base/object_representation.h:503`). Its bits are divided as follows.

| Bits | Meaning | Accessor |
|---|---|---|
| 0–23 | representation id | `OR_GET_MVCC_REPID` masks with `OR_MVCC_REPID_MASK 0x00FFFFFF` (`src/base/object_representation.h:571`; mask at `src/base/object_representation_constants.h:176`) |
| 24–28 | record flags (5 bits) | `OR_GET_RECORD_FLAGS` shifts right by `OR_RECORD_FLAG_SHIFT_BITS 24` and masks with `OR_RECORD_FLAG_MASK 0x1f` (`src/base/object_representation.h:577`; constants at `src/base/object_representation_constants.h:159`) |
| 29–30 | variable-offset entry width | `OR_GET_OFFSET_SIZE` compares against `OR_OFFSET_SIZE_FLAG 0x60000000` (`src/base/object_representation.h:547`; constants at `src/base/object_representation_constants.h:153`) |
| 31 | bound-bit array present | `OR_GET_BOUND_BIT_FLAG` masks with `OR_BOUND_BIT_FLAG 0x80000000` (`src/base/object_representation.h:544`, `:534`) |

Offset-width encoding is `01` → 1 byte, `10` → 2 bytes, `11` → 4 bytes.

Of the five record-flag bits, three are MVCC flags and one is a feat-oos format marker:

- `OR_MVCC_FLAG_VALID_INSID 0x01` — record carries an MVCC insert id
- `OR_MVCC_FLAG_VALID_DELID 0x02` — record carries an MVCC delete id (when clear, the slot holds CHN instead)
- `OR_MVCC_FLAG_VALID_PREV_VERSION 0x04` — record carries the previous-version LSA
- `OR_RECORD_FLAG_HAS_OOS 0x08` — record contains at least one out-of-row variable column. Explicitly documented as *not* affecting header size (`src/base/object_representation_constants.h:173`).

All four are defined at `src/base/object_representation_constants.h:165`–`:174`. The header-size computation masks with `OR_RECORD_MVCC_FLAG_MASK 0x07` first (`OR_GET_MVCC_FLAGS`, `src/base/object_representation.h:581`), which is how the OOS bit stays out of the size lookup.

The writer that composes this word is `heap_attrinfo_transform_header_to_disk` (`src/storage/heap_file.c:12795`): it starts from `attr_info->last_classrepr->id` (`:12803`), ORs in `OR_BOUND_BIT_FLAG` when the class has at least one fixed attribute (`:12806`–`:12809`), sets the offset-size bits via `OR_SET_VAR_OFFSET_SIZE` (`:12812`), ORs `OR_RECORD_FLAG_HAS_OOS` when applicable (`:12819`–`:12822`), and finally ORs the MVCC flags (`:12828`, `:12843`).

Note the asymmetry between `OR_GET_REPID` (`src/base/object_representation.h:541`), which masks out only the bound-bit and offset-size bits and would therefore leave record flags contaminating the value, and `OR_GET_MVCC_REPID`, which masks to 24 bits. **A reimplementer must use the 24-bit mask.** The public accessor `or_rep_id` does exactly that (`src/object/object_representation.c:281`, returning `OR_GET_MVCC_REPID` at `:293`).

### 1.3 Header size is flag-driven

The header layout is a fixed sequence with optional members:

```
+0   repid_and_flags   4 bytes   (always)
+4   CHN               4 bytes   (always; holds delete-MVCCID slot semantics, see below)
+8   MVCC insert id    8 bytes   (only if VALID_INSID)
+..  MVCC delete id    8 bytes   (only if VALID_DELID)
+..  prev version LSA  8 bytes   (only if VALID_PREV_VERSION)
```

The offset formulas are `OR_CHN_OFFSET = 4` (`src/base/object_representation.h:509`), `OR_MVCC_INSERT_ID_OFFSET = 8` (`:512`), `OR_MVCC_DELETE_ID_OFFSET(flags)` (`:515`), and `OR_MVCC_PREV_VERSION_LSA_OFFSET(flags)` (`:519`). Rather than recompute, the engine indexes a lookup table by the 3 MVCC flag bits:

```c
int mvcc_header_size_lookup[8] = { ... };
```

at `src/object/object_representation.c:70`, documented with the flag-to-size table in the comment at `:56`–`:69`, and read by `or_header_size` (`src/object/object_representation.c:5771`) which is what the `OR_HEADER_SIZE` macro expands to (`src/base/object_representation.h:497`). The resulting sizes are 8, 16, 16, 24, 16, 24, 24, 32 for flag values 0–7. Named constants: `OR_NON_MVCC_HEADER_SIZE 8`, `OR_MVCC_MIN_HEADER_SIZE 8`, `OR_MVCC_INSERT_HEADER_SIZE 16`, `OR_MVCC_MAX_HEADER_SIZE 32` (`src/base/object_representation_constants.h:142`–`:150`).

Three practical consequences:

1. A freshly inserted MVCC row has flags `INSID` only, so header size 16. An updated row has `INSID | PREV_VERSION`, so 24. This is set explicitly in `heap_attrinfo_transform_header_to_disk` at `src/storage/heap_file.c:12828` (insert) and `:12843` (update).
2. MVCC-disabled classes (notably the root class) get the 8-byte non-MVCC header (`src/storage/heap_file.c:12858`–`:12868`), with CHN in the second word rather than a dummy.
3. **The header can grow in place after the record is written.** When a delete sets `VALID_DELID`, `or_mvcc_set_header` compares old and new sizes from the same lookup table and memmoves the entire body (`src/base/object_representation_sr.c:4319`–`:4332`, using `HEAP_MOVE_INSIDE_RECORD`). Space for this is pre-reserved at insert time as `mvcc_extra` (`heap_attrinfo_transform_to_disk_internal`, `src/storage/heap_file.c:13284`, `:13290`, checked at `:13214`). For a reader this is benign — everything downstream is expressed relative to the header end — but it means the header size of a record on disk is not predictable from the class, only from the record's own flag bits.

### 1.4 Variable-offset table

Immediately after the header comes the variable-offset table, present only when the class has variable attributes. It holds `n_variable + 1` entries of the width named by the offset-size bits, then pads to `INT_ALIGNMENT`:

```c
#define OR_VAR_TABLE_SIZE_INTERNAL(vars, offset_size) \
  (((vars) == 0) ? 0 : DB_ALIGN ((offset_size * ((vars) + 1)), INT_ALIGNMENT))
```

at `src/base/object_representation.h:466`. The table starts at `OR_HEADER_SIZE(obj)` (`OR_GET_OBJECT_VAR_TABLE`, `src/base/object_representation.h:592`).

Entry *i* is the byte offset of variable attribute *i* **measured from the end of the header**, i.e. from the start of the offset table. The absolute offset within the record is therefore `OR_HEADER_SIZE(obj) + entry[i]` — exactly what `OR_VAR_OFFSET` computes (`src/base/object_representation.h:598`). The writer confirms the base: `length = CAST_BUFLEN (*ptr_varvals - buf->buffer - header_size)` in `heap_attrinfo_transform_variable_to_disk` (`src/storage/heap_file.c:13031`), with the comment "compute the variable offsets relative to the end of the header (beginning of variable table)" at `:13024`.

The last entry (`index == n_variable`) is the offset to the end of the object, written separately in `heap_attrinfo_transform_columns_to_disk` at `src/storage/heap_file.c:13200`–`:13209`. Because of this sentinel, an attribute's length is normally `entry[i+1] - entry[i]`, and a zero-length entry means SQL NULL (`OR_VAR_IS_NULL`, `src/base/object_representation.h:603`).

**feat-oos: the low 2 bits of every entry are flags.** The block at `src/base/object_representation.h:445`–`:456` defines:

```c
#define OR_VAR_BIT_OOS 0x1
#define OR_VAR_BIT_LAST_ELEMENT 0x2
#define OR_VAR_FLAG_MASK 0x3
#define OR_GET_VAR_OFFSET(length) ((int) (length) & (~OR_VAR_FLAG_MASK))
```

and `OR_VAR_TABLE_ELEMENT_OFFSET_INTERNAL` applies `OR_GET_VAR_OFFSET` to every read (`src/base/object_representation.h:476`). On `develop` the same macro returns the raw value with no masking (`/home/vimkim/gh/cb/develop/src/base/object_representation.h:447`). This works because all variable values are written with `INT_ALIGNMENT` padding (see 4.2), so the low 2 bits of a true offset were always zero; feat-oos repurposes them. The invariant is asserted at `src/storage/heap_file.c:13132`. `OR_VAR_BIT_LAST_ELEMENT` marks the sentinel entry; `OR_VAR_BIT_OOS` marks a column whose value is a 16-byte out-of-row stub.

A reader must also note the raw-vs-masked distinction in the engine's own helpers: `heap_recdes_get_var_offset_entry` returns the **raw** entry including flags (`src/storage/heap_file.c:10397`), and the caller tests `OR_IS_OOS(entry)` on it while using the masked `OR_VAR_OFFSET` for the pointer (`heap_attrvalue_point_variable`, `src/storage/heap_file.c:10535`, `:10543`, `:10544`).

Offset width is chosen by payload size, not by column count: `heap_attrinfo_get_record_header_size` starts at 1 byte and escalates to 2 when `header_size + payload_size > OR_MAX_BYTE` (127) and to 4 when it exceeds `OR_MAX_SHORT` (32767) (`src/storage/heap_file.c:12272`–`:12289`). Class objects are a special case: they are always written with 4-byte entries, and both `or_class_rep_dir` and `or_get_current_representation` assert it (`src/base/object_representation_sr.c:736`, `:2436`).

### 1.5 Fixed-attribute area and bound bits

The fixed area begins immediately after the (aligned) offset table:

```c
#define OR_FIXED_ATTRIBUTES_OFFSET_BY_OBJ(obj, nvars) \
  (OR_HEADER_SIZE(obj) + OR_VAR_TABLE_SIZE_INTERNAL(nvars, OR_GET_OFFSET_SIZE(obj)))
```

at `src/storage/heap_file.c:108` (the equivalent generic macro is `OR_FIXED_ATTRIBUTES_OFFSET`, `src/base/object_representation.h:489`). Each fixed attribute sits at `fixed_area_start + attr->location`, where `location` is the precomputed byte offset assigned when the representation was built (`src/base/object_representation_sr.c:2577`). The reader is `heap_attrvalue_point_fixed` (`src/storage/heap_file.c:10372`, pointer arithmetic at `:10383`).

The total fixed-area width is `rep->fixed_length`, which is the sum of each fixed attribute's `tp_domain_disk_size` rounded up with `DB_ATT_ALIGN` (`src/base/object_representation_sr.c:2578`, `:3093`; `DB_ATT_ALIGN` at `src/base/memory_alloc.h:100`). The same computation happens at schema time in `classobj_...` (`src/object/class_object.c:7444`–`:7448`).

The bound-bit array follows the fixed area:

```c
#define OR_GET_BOUND_BITS(obj, nvars, fsize) \
  (char *) (((char *) (obj)) + OR_HEADER_SIZE(...) + OR_VAR_TABLE_SIZE_INTERNAL(...) + (fsize))
```

at `src/base/object_representation.h:650`. Its width is `OR_BOUND_BIT_BYTES(count) = ((count + 31) >> 5) * 4` — i.e. rounded up to whole 4-byte words (`src/base/object_representation.h:640`), where `count` is the number of *fixed* attributes (`n_attributes - n_variable`, as used at `src/storage/heap_file.c:13168`).

Bit indexing is per-element with `OR_BOUND_BIT_MASK(e) = 1 << (e & 7)` and byte `bitptr + (e >> 3)` (`src/base/object_representation.h:642`–`:648`). The test a reader needs is:

```c
#define OR_FIXED_ATT_IS_UNBOUND(obj, nvars, fsize, position) \
  (OR_GET_BOUND_BIT_FLAG (obj) && !OR_GET_BOUND_BIT (OR_GET_BOUND_BITS (obj, nvars, fsize), position))
```

at `src/base/object_representation.h:660`. Note that if the bound-bit flag in bit 31 is clear, *every* fixed attribute is treated as bound. `position` is the attribute's index over all attributes in representation order, assigned as `att->position = i` in `or_get_current_representation` (`src/base/object_representation_sr.c:2542`); since fixed attributes come first, `position == location`'s ordinal for fixed columns.

Variable attributes have no bound bit — a NULL variable attribute is encoded as a zero-length offset-table span (1.4).

### 1.6 Assembled layout

```
offset 0                          repid_and_flags (4, big-endian)
       4                          CHN (4)   [or delete-MVCCID slot, per flags]
       8                          MVCC insid (8)          if VALID_INSID
       ...                        MVCC delid (8)          if VALID_DELID
       ...                        prev version LSA (8)    if VALID_PREV_VERSION
  H = or_header_size()            variable offset table: (n_variable+1) entries
                                    of 1/2/4 bytes, padded to 4
  H + VT                          fixed attribute area: rep->fixed_length bytes
  H + VT + fixed_length           bound bits: OR_BOUND_BIT_BYTES(n_fixed) bytes
  H + entry[0]                    variable value 0
  H + entry[1]                    variable value 1
       ...
  H + entry[n_variable]           end of object
```

where `VT = OR_VAR_TABLE_SIZE_INTERNAL(n_variable, offset_size)`. The first variable value starts right after the bound bits — `ptr_varvals` is initialized to exactly `bound_bits + OR_BOUND_BIT_BYTES(n_fixed)` in `heap_attrinfo_transform_columns_to_disk` (`src/storage/heap_file.c:13165`–`:13169`).

---

## 2. Resolving a record's class from raw volumes only

### 2.1 The direct answer: slot 0 of the page

Every `PAGE_HEAP` page reserves slot 0 for metadata: `#define HEAP_HEADER_AND_CHAIN_SLOTID 0` (`src/storage/heap_file.h:62`). On the heap's own header page that slot holds `HEAP_HDR_STATS`; on every other heap page it holds `HEAP_CHAIN`.

Both structs deliberately start with the class OID, each with the comment `/* the first must be class_oid */`:

- `struct heap_hdr_stats` at `src/storage/heap_file.c:199`, `class_oid` declared at `:202` under the comment at `:201`
- `struct heap_chain` at `src/storage/heap_file.c:283`, `class_oid` declared at `:286` under the comment at `:285`

That is why one function works on both: `heap_get_class_oid_from_page` reads slot 0, casts to `HEAP_CHAIN *`, and copies field 0 (`src/storage/heap_file.c:20471`, cast at `:20483`). Its one special case is that a NULL class OID means the root class and is substituted with `oid_Root_class_oid` (`:20490`–`:20494`).

**Discriminating the two structs is by record length, not by any tag.** `sizeof(HEAP_HDR_STATS)` is 1160 and `sizeof(HEAP_CHAIN)` is 40, and the engine branches on exactly that: `heap_page_is_bestspace` returns early when `recdes.length != sizeof (HEAP_CHAIN)` (`src/storage/heap_file.c:2800`), and so does `heap_page_is_not_in_heap` (`:2825`). volmap already relies on the same discriminator with the same constants (`/home/vimkim/temp/volmap/src/format/heap.rs:6`, `:7`, `:280`–`:288`).

Field offsets, cross-validated between the struct declaration and volmap's working decoder:

| `HEAP_HDR_STATS` field | Offset | Source |
|---|---:|---|
| `class_oid` | 0 | `src/storage/heap_file.c:202`; volmap `heap.rs:302` region |
| `ovf_vfid` (multipage overflow file) | 8 | `src/storage/heap_file.c:204`; volmap `heap.rs` `base + 8` |
| `next_vpid` | 16 | `src/storage/heap_file.c:206` |
| `last_vpid` | 24 | `src/storage/heap_file.c:207`; volmap `heap.rs:295` |
| `oos_vfid` (**feat-oos only**) | 32 | `src/storage/heap_file.c:208`; volmap `heap.rs:303` |
| `unfill_space` | 40 | `src/storage/heap_file.c:209`; volmap `heap.rs:296` |
| `num_pages` | 44 | `src/storage/heap_file.c:211`; volmap `heap.rs:297` |

| `HEAP_CHAIN` field | Offset | Source |
|---|---:|---|
| `class_oid` | 0 | `src/storage/heap_file.c:286`; volmap `heap.rs:315` |
| `prev_vpid` | 8 | `src/storage/heap_file.c:287`; volmap `heap.rs:316` |
| `next_vpid` | 16 | `src/storage/heap_file.c:288`; volmap `heap.rs:317` |
| `max_mvccid` | 24 | `src/storage/heap_file.c:289`; volmap `heap.rs:318` |
| `flags` | 32 | `src/storage/heap_file.c:290`; volmap `heap.rs:310` |

Chain flags are `HEAP_PAGE_FLAG_BESTSPACE 0x00000001`, `HEAP_PAGE_FLAG_NOT_IN_HEAP 0x00000002`, and a 2-bit vacuum status in `0xC0000000` (`src/storage/heap_file.c:234`–`:238`).

The chain's class OID is copied from the header's at page-allocation time: `new_page_chain.class_oid = heap_hdr->class_oid` in `heap_vpid_alloc` (`src/storage/heap_file.c:2959`, with `max_mvccid`/`flags` reset at `:2962`–`:2963`) and the same for bestspace pages in `heap_create_bestspace` (`:3950`). New pages are initialized by `heap_vpid_init_new` (`src/storage/heap_file.c:2866`), which inserts the chain as a `REC_HOME` record and hard-checks that it landed in slot 0 (`:2891`).

**Slot 0 is type `REC_HOME` and is otherwise indistinguishable from a user record.** Any enumerator must skip it explicitly; the engine does, e.g. `if (slotid == HEAP_HEADER_AND_CHAIN_SLOTID) continue;` at `src/transaction/locator_sr.c:13061`, and `HEAP_ISJUNK_OID` treats slot 0 as junk (`src/storage/heap_file.h:66`).

### 2.2 The file-header path, as a cross-check

The file header page carries the same information in `FILE_HEAP_DES`. `struct file_header` (`src/storage/file_manager.c:90`) places the 64-byte type union `FILE_DESCRIPTORS descriptor` at byte offset 40 of the header page's user area (declared at `:96`; `FILE_DESCRIPTORS_SIZE 64` and the union at `src/storage/file_manager.h:137`–`:149`). volmap reads the heap class OID at exactly `+40` (`/home/vimkim/temp/volmap/src/format/file_table.rs:232`).

Descriptor member order differs by file type, which is a live trap:

| File type | Descriptor struct | `class_oid` offset within descriptor |
|---|---|---:|
| `FILE_HEAP`, `FILE_HEAP_REUSE_SLOTS` | `FILE_HEAP_DES { OID class_oid; HFID hfid; }` (`src/storage/file_manager.h:83`) | +0 |
| `FILE_MULTIPAGE_OBJECT_HEAP` | `FILE_OVF_HEAP_DES { HFID hfid; OID class_oid; }` (`:91`) | +12 |
| `FILE_OOS` (**feat-oos only**) | `FILE_OOS_DES { HFID hfid; OID class_oid; }` (`:99`) | +12 |
| `FILE_BTREE` | `FILE_BTREE_DES { OID class_oid; int attr_id; }` (`:107`) | +0 |
| `FILE_BTREE_OVERFLOW_KEY` | `FILE_OVF_BTREE_DES { BTID btid; OID class_oid; }` (`:115`) | +12 |

Heap creation writes both places: the descriptor via `des.heap.class_oid = *class_oid` then `file_descriptor_update` (`heap_create_internal`, `src/storage/heap_file.c:4874`–`:4876`) and the header slot via `heap_hdr.class_oid = *class_oid` (`:4892`) followed by `spage_insert` with a hard check on slot 0 (`:4929`–`:4930`).

### 2.3 Finding the header page, and the two-level HFID

`FILE_GET_HEADER_VPID` shows that a file's VFID *is* its header page's VPID: `volid = vfid->volid; pageid = vfid->fileid` (`src/storage/file_manager.c:196`, matching `file_create` at `:3501`–`:3502`).

For heaps there are two distinct pages and it is easy to conflate them:

- `hfid.vfid.fileid` is the pageid of the **file header page**, page type `PAGE_FTAB`, holding `FILE_HEADER` at byte 0 (`file_create`, `src/storage/file_manager.c:3514` sets `PAGE_FTAB`).
- `hfid.hpgid` is the pageid of the **heap header page**, page type `PAGE_HEAP`, slot 0 = `HEAP_HDR_STATS`. It is the file's sticky first page, allocated separately by `file_alloc_sticky_first_page` (`heap_create_internal`, `src/storage/heap_file.c:4849`), asserted to be on the file's own volume (`:4856`–`:4859`) and assigned at `:4871`.

**There is no reverse page→file map on disk.** The disk sector table records reserved-vs-free only; ownership lives exclusively in each file's own sector tables. Even CUBRID's own membership check demands the VFID up front: `file_check_vpid (thread_p, const VFID *vfid, const VPID *vpid_lookup)` computes `VSID_FROM_VPID` and then searches *that file's* partial and full tables (`src/storage/file_manager.c:6940`). The file tracker is a VFID→type list with no page ids: `FILE_TRACK_ITEM { INT32 fileid; INT16 volid; INT16 type; FILE_TRACK_METADATA metadata; }` (`src/storage/file_manager.c:514`), registered from `file_create` (`:3850`).

So an offline reader that wants page→class by file must build the index itself: enumerate files, and for each walk its partial-sector table (items are `FILE_PARTIAL_SECTOR { VSID vsid; FILE_ALLOC_BITMAP page_bitmap; }`, 16 bytes, `src/storage/file_manager.h:172`, with `FILE_ALLOC_BITMAP` a `UINT64` at `:163` giving one bit per page) expanding each 64-bit bitmap into 64 page ids, plus its full-sector table (bare 8-byte `VSID` items). Both tables are `FILE_EXTENSIBLE_DATA` regions in the header page tail at `FILE_HEADER.offset_to_partial_ftab` / `offset_to_full_ftab` (`src/storage/file_manager.c:119`–`:120`; accessor macros at `:199`–`:211`; `struct file_extensible_data` at `:231`, header 16 bytes, chained via `vpid_next`).

For interpreting a single clicked page, slot 0 is strictly cheaper and needs no index.

### 2.4 Caveats

- **Root-class and boot heaps store a NULL class OID on disk.** The three system heaps are created with `class_oid == NULL` (`src/transaction/boot_sr.c:4989`, `:4995`, `:5002`), `heap_create_internal` substitutes a null OID (`class_oid = &null_oid`, `src/storage/heap_file.c:4795`–`:4798`), and the mapping to the real root-class OID happens only at read time (`src/storage/heap_file.c:20490`). An offline reader must apply the same substitution, which requires knowing the root class OID — recoverable from `boot_dbparm.rootclass_hfid` (see 6.1), but not from the page.
- **Overflow pages carry no class OID.** `REC_BIGONE` payloads live in a separate `FILE_MULTIPAGE_OBJECT_HEAP` file whose pages are `PAGE_OVERFLOW` and are *not* slotted: `OVERFLOW_FIRST_PART { VPID next_vpid; int length; char data[1]; }` / `OVERFLOW_REST_PART { VPID next_vpid; char data[1]; }` (`src/storage/overflow_file.h:37`–`:51`). The owning class is only in the file descriptor.
- **`PAGE_FTAB` pages inside a heap file's sectors carry no chain.** See 5.4.
- **Bestspace pages** carry a valid chain (and therefore a valid class OID) but hold no user rows; slot 1 (`HEAP_BESTSPACE_ENTRIES_SLOTID`, `src/storage/heap_file.c:255`) is a `bestspace_entry[]` array inserted as a `REC_HOME` record (`:4014`–`:4021`). Detect via `HEAP_PAGE_FLAG_BESTSPACE`.
- **A mark-deleted heap still names the dropped class** on every page and in its descriptor until the file is reused or destroyed (`file_tracker_item_mark_heap_deleted`, `src/storage/file_manager.c:10599`, sets `item->metadata.heap.is_marked_deleted = true` at `:10609`). On reuse, `heap_reuse` (`src/storage/heap_file.c:5116`) rewrites `class_oid` into the header (`:5252`) and every chain, resetting `max_mvccid` and `flags` (`:5263`–`:5265`), and `file_tracker_item_reuse_heap` (`src/storage/file_manager.c:10448`) rewrites the file descriptor (`:10529`–`:10530`). Descriptor and pages therefore never disagree — but a stale *class* is possible before reuse.

---

## 3. How the catalog stores class representations

An important framing correction before the details: **the system catalog is not on the path from a heap record to its attribute values.** Section 3.5 shows that the engine decodes instances against representations parsed out of the class object's own heap record. The catalog's `DISK_REPR`/`DISK_ATTR` records exist to serve the query optimizer's statistics. This section documents both, but a reader implementing interpretation should read 3.5 and 4, not 3.2–3.4.

### 3.1 Entry point: the class record's `rep_dir`, not an extendible hash

The catalog is a `FILE_CATALOG` file on the first permanent volume. Its identity is `struct ctid { VFID vfid; EHID xhid; PAGEID hpgid; }` (`src/storage/system_catalog.h:45`), created by `catalog_create` (`src/storage/system_catalog.c:2626`), which calls `xehash_create` at `:2635`, `file_create_with_npages (FILE_CATALOG, ...)` at `:2640`, and records the sticky first page as `hpgid` at `:2663`. The CTID itself is persisted in `boot_dbparm.ctid` (`src/transaction/boot_sr.c:128`) and pinned to `LOG_DBFIRST_VOLID` before creation (`src/transaction/boot_sr.c:4960`–`:4961`).

**That extendible hash is dead.** `xehash_create` at `src/storage/system_catalog.c:2635` is the only write-side reference to `catalog_Id.xhid`; there are no `ehash_insert`/`ehash_search` calls against it anywhere in `system_catalog.c`, and the one `ehash_delete` sits inside a `#if 0` block opened at `:2223`. `CATALOG_DIR_REPR_KEY` survives at `src/storage/system_catalog.h:42` but is now only a volatile-hashmap key discriminator (`src/storage/system_catalog.c:5747`). An offline reader that walks the EHT will find nothing.

The real mapping lives in the class object's heap record. `catalog_get_rep_dir` (`src/storage/system_catalog.c:1832`) tries the volatile hashmap first, then falls back to the durable path at `:1889`–`:1893`: read the class record from the root-class heap, then `or_class_rep_dir`. That accessor is:

```c
ptr = (char *) record->data
      + OR_FIXED_ATTRIBUTES_OFFSET (record->data, ORC_CLASS_VAR_ATT_COUNT)
      + ORC_REP_DIR_OFFSET;
OR_GET_OID (ptr, rep_dir_p);
```

at `src/base/object_representation_sr.c:732` (body `:738`–`:740`), with `ORC_REP_DIR_OFFSET = 8` (`src/base/object_representation.h:757`) and `ORC_CLASS_VAR_ATT_COUNT = ORC_LAST_INDEX` = 17 (`src/base/object_representation.h:796`, enum list at `:774`–`:794`). The stored value is not a real object OID; it is a `{volid, pageid, slotid}` locator into the catalog file, manufactured in `catalog_insert_representation_item` (`src/storage/system_catalog.c:2066`–`:2068`).

### 3.2 Catalog pages are slotted pages

`catalog_initialize_new_page` (`src/storage/system_catalog.c:598`) sets `PAGE_CATALOG` at `:611`, calls `spage_initialize (..., ANCHORED_DONT_REUSE_SLOTS, MAX_ALIGNMENT, SAFEGUARD_RVSPACE)` at `:612`, and inserts the 16-byte page header, hard-checking that it landed in `CATALOG_HEADER_SLOT` (`= 0`, `src/storage/system_catalog.c:59`) at `:622`. `ANCHORED_DONT_REUSE_SLOTS` means stored `(pageid, slotid)` locators stay valid, which is what makes offline navigation possible at all.

The 16-byte page header, all big-endian (offsets at `src/storage/system_catalog.c:64`–`:69`, readers at `:72`–`:82`, writers at `:85`–`:95`):

| Offset | Width | Field |
|---:|---:|---|
| 0 | 4 | `overflow_page_id.pageid` |
| 4 | 2 | `overflow_page_id.volid` |
| 8 | 4 | `dir_count` |
| 12 | 4 | `is_overflow_page` (an `int` holding a bool) |

Bytes 6–7 are unused. `dir_count` is a page-selection heuristic only, maintained by `catalog_adjust_directory_count` (`src/storage/system_catalog.c:1975` region).

### 3.3 The representation directory

A directory record is exactly two `CATALOG_REPR_ITEM`s (32 bytes) and holds at most two live items. Item offsets at `src/storage/system_catalog.c:140`–`:146`, readers at `:148`–`:161`, transformers `catalog_get_repr_item_from_record` at `:531` and `catalog_put_repr_item_to_record` at `:540`:

| Offset | Width | Field |
|---:|---:|---|
| 0 | 4 | `page_id.pageid` |
| 4 | 2 | `page_id.volid` |
| 8 | 2 | `repr_id` (INT16) |
| 10 | 2 | `slot_id` |
| 12 | 1 | item count — meaningful only in item 0 |

`CATALOG_REPR_ITEM_SIZE` is 16 (`src/storage/system_catalog.c:146`). `catalog_get_representation_record` asserts `record_p->length == CATALOG_REPR_ITEM_SIZE * 2` and a count of 1 or 2 (`src/storage/system_catalog.c:1913`, asserts at `:1946`–`:1947`). Lookup is a linear scan over `count` fixed-stride items in `catalog_find_representation_item_position` (`src/storage/system_catalog.c:2011`).

`repr_id == NULL_REPRID` (`-1`, `src/storage/storage_common.h:607`; stored as INT16, so `0xFFFF` on disk) marks the `CLS_INFO` entry, not a representation. Readers set that sentinel deliberately (`catalog_get_class_info`, `src/storage/system_catalog.c:4143`–`:4145`; `catalog_get_rep_dir`, `:1849`–`:1851`) and directory enumerators skip it (`catalog_get_last_representation_id`, `src/storage/system_catalog.c:4301`, scan at `:4323`–`:4326`).

In practice the directory holds one representation at a time: `catalog_drop_old_representations` rewrites the record as `[class-info, last-repr]` (`src/storage/system_catalog.c:3539`, rewrite at `:3659`–`:3679`), which is why the "1 or 2" assertion holds.

### 3.4 CLS_INFO, DISK_REPR, DISK_ATTR, BTREE_STATS on disk

All four are big-endian `OR_PUT_*`/`OR_GET_*` encodings. `CLS_INFO` is 56 bytes (offsets at `src/storage/system_catalog.c:132`–`:138`; transformers `catalog_get_class_info_from_record` at `:504`, `catalog_put_class_info_to_record` at `:517`; struct `struct cls_info` at `src/storage/system_catalog.h:96`):

| Offset | Width | Field |
|---:|---:|---|
| 0 | 4 | `ci_hfid.hpgid` |
| 4 | 4 | `ci_hfid.vfid.fileid` |
| 8 | 4 | `ci_hfid.vfid.volid` — written as INT32 by `OR_PUT_HFID`, not INT16 (`src/base/object_representation.h:321`) |
| 12 | 4 | `ci_tot_pages` |
| 16 | 4 | `ci_tot_objects` |
| 20 | 4 | `ci_time_stamp` |
| 24 | 8 | `ci_rep_dir` OID (pageid@24, slotid@28, volid@30, per `OR_OID_*` at `src/base/object_representation_constants.h:68`–`:70`) |
| 32 | 24 | reserved, explicitly zeroed at insert (`src/storage/system_catalog.c:3100`) |

`DISK_REPR` is a 56-byte header (`struct disk_representation` at `src/storage/system_catalog.h:63`; offsets at `src/storage/system_catalog.c:98`–`:103`; `catalog_get_disk_representation` at `:399`, `catalog_put_disk_representation` at `:414`): `id`@0, `n_fixed`@4, `fixed_length`@8, `n_variable`@12, `reserved_1`@16 (written as literal 0). **Bytes 20–55 are never written and never read** — the writer's buffer is `db_private_alloc`'d without a memset (`src/storage/system_catalog.c:2887`), so they contain heap garbage. Do not interpret them.

`DISK_ATTR` is 88 bytes (`struct disk_attribute` at `src/storage/system_catalog.h:80`; offsets at `src/storage/system_catalog.c:109`–`:117`; `catalog_get_disk_attribute` at `:426`, `catalog_put_disk_attribute` at `:465`):

| Offset | Width | Field |
|---:|---:|---|
| 0 | 4 | `id` |
| 4 | 4 | `location` — byte offset in the fixed area for fixed attrs, index into the offset table for variable attrs |
| 8 | 4 | `type` (DB_TYPE as int) |
| 12 | 4 | `val_length` — byte length of the default-value blob that follows |
| 16 | 4 | `position` |
| 20 | 8 | `classoid` |
| 28 | 4 | `n_btstats` |
| 32–79 | 48 | never written, never read |
| 80 | 8 | `ndv` (INT64) |

Semantics of `location` and `position` are documented on the struct at `src/storage/system_catalog.h:80`–`:82` and match the class-record-derived values in section 1.5.

`BTREE_STATS` is 80 bytes (offsets at `src/storage/system_catalog.c:119`–`:127`; `catalog_get_btree_statistics` at `:441`): `btid`@0 (12 bytes, `OR_BTID_ALIGNED_SIZE`), `leafs`@12, `pages`@16, `height`@20, `keys`@24, `has_function`@28, `pkeys[0..7]`@32, reserved@64. **Only `pkeys_size` entries are valid, and `pkeys_size` is not in the record** — `catalog_fetch_btree_statistics` must fix the index's B-tree root page and parse its packed key domain to learn it (`src/storage/system_catalog.c:1501`, BTID read at `:1521`, root page fix at `:1533`, `btree_get_root_header` at `:1543`, `pkeys_size` derived at `:1551`–`:1557`). An offline reader cannot fully decode `BTREE_STATS` without also parsing B-tree roots.

Record body layout, driven by `catalog_add_representation` (`src/storage/system_catalog.c:2815`, loop at `:2921`–`:2932`) and mirrored by `catalog_assign_attribute` (`:3804`):

```
[DISK_REPR 56]
[ATTR 88][default value: val_length bytes, NO padding][BTREE_STATS 80] x n_btstats
[ATTR 88][default value ...][BTREE_STATS 80] x n_btstats
...   fixed attributes first, then variable attributes
```

Default values are `memcpy`'d at exactly `val_length` bytes with no alignment insert (`catalog_store_attribute_value`, `src/storage/system_catalog.c:1248`; reader `catalog_fetch_attribute_value`, `:1447`), and `val_length` is an arbitrary byte count originating in `or_get_default_value` (`or_get_default_value`, `src/base/object_representation_sr.c:1384`, assignment at `:1413`). **A reader must therefore track a running byte cursor and never assume 8-byte alignment inside the record body.**

Records spanning multiple catalog pages: there is no `CATALOG_MAX_RECORD_SIZE`. The writer budget is `spage_max_space_for_new_record() - CATALOG_MAX_SLOT_ID_SIZE` (12, `src/storage/system_catalog.c:60`), applied at `:2901` and again per continuation page at `:1156`. `catalog_write_unwritten_portion` (`:1164`) flushes the page before any fixed-size item would straddle, so `DISK_REPR`, `DISK_ATTR`, and `BTREE_STATS` blocks are never split; only a default *value* can be, via the explicit loop in `catalog_store_attribute_value` (`:1272`–`:1296`). The reader's continuation rule, in `catalog_get_record_from_page` (`src/storage/system_catalog.c:1324`), is: **read the current page's header slot, take its `overflow_page_id`, and continue at slot 1 of that page** (`spage_get_record` on `CATALOG_HEADER_SLOT` at `:1335`, the two `CATALOG_GET_PGHEADER_OVFL_PGID_*` reads at `:1341`–`:1342`, `slotid = 1` at `:1343`, terminating on `NULL_PAGEID` at `:1348`). The first fragment's slot comes from the directory item; every subsequent fragment is slot 1.

`catalog_get_representation` (`src/storage/system_catalog.c:3887`) ties it together: resolve the rep_dir from the class record via `catalog_get_dir_oid_from_cache` (`:3904`, body `:5733`), take the S-lock (`:3911`), map `(class_oid, repr_id)` to `(page_id, slot_id)` via `catalog_get_representation_item` (`:3935`, body `:2287`), seed a forward-only `(fragment, offset)` cursor (`:3944`–`:3949`), read the 56-byte header via `catalog_fetch_disk_representation` (`:3962`, body `:1400`), allocate the fixed and variable attribute arrays (`:3985`–`:4012`), then loop `catalog_assign_attribute` over fixed then variable (`:4015`–`:4028`). The whole traversal is a single forward byte stream; there is no random access inside a representation record.

### 3.5 What a live record's reprid actually indexes

The reprid in a heap record header resolves against the **class object's heap record**, not the catalog. `heap_classrepr_get` (`src/storage/heap_file.c:2044`) delegates to `heap_classrepr_get_from_record` (`:1991`), which reads the class record through the root-class heap and calls `or_get_classrep (recdes, reprid)` at `:2018`, returning `or_rep_id (recdes)` as `*last_reprid` at `:2021`.

`or_get_classrep` (`src/base/object_representation_sr.c:3351`) branches three ways: `NULL_REPRID` → `or_get_current_representation` (`:3360`); reprid equal to the class record's own `or_rep_id` → the current representation (`:3365`, `:3369`); otherwise → `or_get_old_representation` (`:3373`), which linearly searches the class record's `ORC_REPRESENTATIONS_INDEX` variable attribute — a packed set of old-representation objects — comparing `OR_GET_INT (fixed + ORC_REP_ID_OFFSET)` (`src/base/object_representation_sr.c:2934`, scan at `:2972`, `ER_CT_UNKNOWN_REPRID` at `:2986`). Sub-layout constants: `ORC_REP_ID_OFFSET 0`, `ORC_REP_FIXED_COUNT_OFFSET 4`, `ORC_REP_VARIABLE_COUNT_OFFSET 8` (`src/base/object_representation.h:830`–`:832`); `ORC_REP_ATTRIBUTES_INDEX 0` (`:837`).

The two representation sets stay keyed identically because both derive the id from the same place — `catalog_insert` does `new_repr_id = (REPR_ID) or_rep_id (record_p)` (`src/storage/system_catalog.c:4358`) and `catalog_update` the same (`:4432`) — but they are parallel structures with different consumers.

`or_get_current_representation` (`src/base/object_representation_sr.c:2414`) is the function a reimplementer should port. Its shape:

1. `ptr = start + OR_FIXED_ATTRIBUTES_OFFSET (record->data, ORC_CLASS_VAR_ATT_COUNT)` (`:2438`) — the class object's own fixed area.
2. `rep->id = or_rep_id (record)` (`:2440`); `fixed_length` from `ORC_FIXED_LENGTH_OFFSET` (`:2441`); `n_fixed`, `n_variable`, `n_shared_attrs`, `n_class_attrs` from `ORC_FIXED_COUNT_OFFSET`, `ORC_VARIABLE_COUNT_OFFSET`, `ORC_SHARED_COUNT_OFFSET`, `ORC_CLASS_ATTR_COUNT_OFFSET` (`:2447`–`:2450`). The `ORC_*` class offsets are the enum at `src/base/object_representation.h:755`–`:770`.
3. `attset = start + OR_VAR_OFFSET (start, ORC_ATTRIBUTES_INDEX)` (`:2493`) — the attributes substructure is variable attribute 5 of the class object (`ORC_ATTRIBUTES_INDEX = 5`, `src/base/object_representation.h:779`), encoded as a set.
4. For each attribute *i*: `diskatt = attset + OR_SET_ELEMENT_OFFSET (attset, i)` (`:2502`), then read the attribute's fixed area at `diskatt + OR_VAR_TABLE_SIZE (ORC_ATT_VAR_ATT_COUNT)` (`:2517`) to get flags@24, type@4, id@0, def_order@12 (`:2519`, `:2538`–`:2541`; offsets at `src/base/object_representation.h:800`–`:808`). The domain comes from the attribute's variable attribute 3 (`ORC_ATT_DOMAIN_INDEX`) via `or_get_domain_and_cache` (`:2570`–`:2571`).
5. `att->position = i` (`:2542`); then `is_fixed = 1, location = offset, offset += tp_domain_disk_size (att->domain)` for `i < n_fixed`, else `is_fixed = 0, location = i - n_fixed` (`:2574`–`:2584`).
6. `rep->fixed_length = DB_ATT_ALIGN (offset - start)` (`:3093` in the sibling `or_get_old_representation` path; the current-representation path takes `fixed_length` from the class record at `:2441`).

The domain substructure's own offsets — `type`@0, `precision`@4, `scale`@8, `codeset`@12, `collation_id`@16, `class`@20 — are at `src/base/object_representation.h:868`–`:873`. Codeset and collation matter for section 4.

Two more class-record accessors a reader will want: `or_class_hfid` reads the HFID from `ORC_HFID_FILEID_OFFSET 16` / `ORC_HFID_VOLID_OFFSET 20` / `ORC_HFID_PAGEID_OFFSET 24` (`src/base/object_representation_sr.c:756`, body `:762`–`:765`), and `or_class_name` returns the class name as variable attribute 0, skipping the varchar length prefix (`src/object/object_representation.c:237`, offset at `:249`, prefix skip at `:261`–`:269`).

---

## 4. Decoding attribute values

### 4.1 The dispatch: fixed vs variable, bound bit, then `data_readval`

The engine's per-attribute reader is `heap_attrvalue_read` (`src/storage/heap_file.c:10639`). Its logic, in order:

1. Skip synthetic deduplicate-key attributes (`:10649`).
2. If the attribute is shared/class-scoped or absent from the record's representation, use the default value from `last_attrepr->default_value` (`:10657`–`:10669`).
3. Otherwise branch on `attrepr->is_fixed` (`:10675`): fixed → `heap_attrvalue_point_fixed`, variable → `heap_attrvalue_point_variable` (`:10677`, `:10681`).
4. Hand the resulting `(data, length)` to `heap_attrvalue_transform_to_dbvalue` (`:10691`), which calls `pr_type->data_readval (&buf, &value->dbvalue, attrepr->domain, raw->length, ...)` (`src/storage/heap_file.c:10613`).

`heap_attrvalue_point_fixed` (`src/storage/heap_file.c:10372`) first applies `OR_FIXED_ATT_IS_UNBOUND (recdes->data, n_variable, fixed_length, attrepr->position)` and returns with a NULL pointer if unbound (`:10375`–`:10380`); otherwise it points at `fixed_area_start + attrepr->location` and sets `raw->length = tp_domain_disk_size (attrepr->domain)` (`:10383`–`:10386`).

`heap_attrvalue_point_variable` (`src/storage/heap_file.c:10518`) returns a NULL pointer when `OR_VAR_IS_NULL` (`:10527`), then points at `recdes->data + OR_VAR_OFFSET (recdes->data, attrepr->location)` (`:10543`). Notably it sets `raw->length = -1` for most types (`:10561`), meaning "let the value's own length prefix decide", and only computes a real span via `OR_VAR_LENGTH` for `BLOB`, `CLOB`, `SET`, `MULTISET`, `SEQUENCE` (`:10551`–`:10559`).

When `raw->data` is NULL, the value is set to SQL NULL by domain (`heap_attrvalue_transform_to_dbvalue`, `src/storage/heap_file.c:10594`–`:10603`).

### 4.2 Which types are fixed and which are variable

This is decided entirely by `variable_p`, the third member of `PR_TYPE` (`src/object/object_primitive.h:87`, constructor argument order at `:115`). Schema build time reads it directly: `if (!att->domain->type->variable_p) fixed_count++; else variable_count++;` (`src/object/class_object.c:7424`–`:7431`), and `fixed_size += tp_domain_disk_size (att->domain)` only for the fixed ones (`:7442`–`:7445`).

From the `PR_TYPE` table in `src/object/object_primitive.c`, with `{name, id, variable_p, size, disksize, alignment}`:

| Type | `variable_p` | Disk size | Definition |
|---|---:|---|---|
| `INTEGER` | 0 | 4 | `src/object/object_primitive.c:922` |
| `SHORT` | 0 | 2 | `:947` |
| `BIGINT` | 0 | 8 | `:972` |
| `FLOAT` | 0 | 4 | `:997` |
| `DOUBLE` | 0 | 8 | `:1022` |
| `TIME` | 0 | `OR_TIME_SIZE` = 4 | `:1047` |
| `TIMESTAMP` | 0 | `OR_UTIME_SIZE` = 4 | `:1072` |
| `DATETIME` | 0 | `OR_DATETIME_SIZE` = 8 | `:1147` |
| `MONETARY` | 0 | `OR_MONETARY_SIZE` = 12 | `:1224` |
| `DATE` | 0 | `OR_DATE_SIZE` = 4 | `:1249` |
| `OBJECT` | 0 | `OR_OID_SIZE` = 8 | `:1282` |
| `ENUMERATION` | 0 | 2 | `:1664` |
| **`CHAR`** | **1** | 0 (length-prefixed) | `:12733` |
| **`NUMERIC`** | **1** | 0 (length-prefixed) | `:1639` |
| `VARCHAR`, `NCHAR`, `VARNCHAR`, `BIT`, `VARBIT` | 1 | 0 | `tp_NChar` aliases `tp_Char` at `:1696`; string/bit tables in the same file |
| `SET`, `MULTISET`, `SEQUENCE` | 1 | 0 | `:1514`, `:1539`, `:1564` |
| `BLOB`, `CLOB` | 1 | 0 | `:1332`, `:1357` |

`CHAR` and `NUMERIC` being variable-region types is counterintuitive and is the most likely source of a wrong reimplementation. The doc comment on `pr_is_variable_type` (`src/object/object_primitive.c:9005`) explains the intent — parameterized types are "the same size for any particular attribute" — but `CHAR` and `NUMERIC` opted out of the fixed region anyway, because both compress or vary by actual value width.

`tp_domain_disk_size` returns -1 for always-variable types and for floating-precision `CHAR`/`BIT` (`src/object/object_domain.c:10896`, checks at `:10898`, `:10903`).

### 4.3 Endianness and the numeric primitives

Every scalar in a record body is big-endian. `OR_PUT_INT`/`OR_GET_INT` are `htonl`/`ntohl` (`src/base/object_representation.h:108`, `:111`), shorts are `htons`/`ntohs` (`:102`, `:105`), 64-bit values go through `swap64` (`:114`, `:121`), and floats/doubles through `htonf`/`ntohf`/`htond`/`ntohd` (`:141`–`:158`). Bytes are raw (`:99`).

This is a real contrast with the page prologue (section 1.1) and with `boot_dbparm` (section 6.1), both of which are native-endian struct images. The two encodings coexist in the same volume file.

Date and time encodings:

- `DB_TIME` is `unsigned int` (`src/compat/dbtype_def.h:835`) holding **seconds since midnight**: `decode_time` computes `seconds = timeval % 60; minutes = (timeval / 60) % 60; hours = (timeval / 3600) % 24` (`src/compat/db_date.c:404`, body `:408`–`:410`). Written by `or_put_time` → `OR_PUT_TIME` → `OR_PUT_INT` (`src/base/object_representation.h:170`), read by `mr_data_readval_time` → `or_get_time` (`src/object/object_primitive.c:3296`; `src/base/object_representation.h:1888`).
- `DB_DATE` is `unsigned int` (`src/compat/dbtype_def.h:852`) holding a **Julian day number**, per `julian_encode` (`src/compat/db_date.c:116`, formula at `:148`, Gregorian correction at `:153`–`:157`). Read via `or_get_date` → `OR_GET_DATE` → `OR_GET_INT` (`src/base/object_representation.h:1989`, `:197`).
- `DB_TIMESTAMP`/`DB_UTIME` is `unsigned int` (`src/compat/dbtype_def.h:840`) holding a Unix timestamp, 4 bytes (`or_get_utime`, `src/base/object_representation.h:1921`).
- `DB_DATETIME` is `{ unsigned int date; unsigned int time; }` (`src/compat/dbtype_def.h:855`), 8 bytes, date at +0 and time at +4 (`OR_DATETIME_DATE 0`, `OR_DATETIME_TIME 4`, `src/base/object_representation_constants.h:111`–`:112`; `OR_GET_DATETIME`, `src/base/object_representation.h:206`). The `time` member here is **milliseconds** since midnight, not seconds — it is the datetime variant, distinct from `DB_TIME`.
- `MONETARY` is 12 bytes: `type` as a big-endian int at +0 and `amount` as a big-endian double at +4 (`OR_MONETARY_TYPE 0`, `OR_MONETARY_AMOUNT 4`, `src/base/object_representation_constants.h:121`–`:122`; `OR_GET_MONETARY` at `src/base/object_representation.h:234`, which `memcpy`s the double precisely because +4 is not 8-aligned). Read by `mr_data_readval_money` → `or_get_monetary` (`src/object/object_primitive.c:4552`; `src/object/object_representation.c:566`). Valid currency codes are enumerated in `or_put_monetary` (`src/object/object_representation.c:506`, switch at `:514`).
- `OBJECT`/`OID` is 8 bytes: pageid@0 (4), slotid@4 (2), volid@6 (2) (`OR_OID_*`, `src/base/object_representation_constants.h:67`–`:70`).

### 4.4 String encoding: the length prefix and LZ4 compression

`VARCHAR` and `CHAR` share one encoding, written by `or_put_varchar_internal` (`src/object/object_representation.c:788`) with `align == INT_ALIGNMENT` on the heap path (`mr_data_writeval_string` → `mr_writeval_string_internal (buf, value, INT_ALIGNMENT)`, `src/object/object_primitive.c:10796`; `mr_data_writeval_char` likewise at `:12168`).

The prefix rule (`src/object/object_representation.c:797`–`:805`, mirrored by `or_get_varchar_compression_lengths` at `src/base/object_representation.h:2160`):

- If `charlen < OR_MINIMUM_STRING_LENGTH_FOR_COMPRESSION` (255, `src/base/object_representation.h:1421`), the prefix is **one byte holding the length**, followed by that many raw bytes.
- Otherwise the prefix is **one byte equal to 255 (0xFF)**, followed by two big-endian 4-byte ints: compressed length, then decompressed length (`:859`–`:869`). A compressed length of 0 means compression was skipped or unprofitable, and the payload is `decompressed_length` raw bytes; a non-zero compressed length means the payload is `compressed_length` LZ4 bytes.

The size accounting is `or_varchar_length_internal` (`src/base/object_representation.h:2347`), whose comment at `:2357`–`:2364` states the encoding explicitly. With `INT_ALIGNMENT`, it adds **one byte for a NUL terminator** and then rounds the whole thing up to 4 (`:2368`–`:2374`). That trailing NUL and the 4-byte round-up are what keep every variable offset a multiple of 4 — which is the precondition the feat-oos flag bits depend on.

The reader is `mr_readval_string_internal` (`src/object/object_primitive.c:10964`): it calls `or_get_varchar_compression_lengths` (`:11002`), LZ4-decompresses when `compressed_size > 0` via `pr_get_compressed_data_from_buffer` (`:11029`), otherwise copies `expected_decompressed_size` bytes (`:11067`), and finally skips the alignment remainder with `or_skip_varchar_remainder` (`:11077`). `mr_readval_char_internal` (`:12281`) is the `CHAR` twin. There is also a dedicated `data_readval_string` for reading heap varchar/char outside the normal path, used by unloaddb (`:11100`).

Codeset and collation come from the domain, not the value: `db_make_varchar (value, precision, ..., TP_DOMAIN_CODESET (domain), TP_DOMAIN_COLLATION (domain))` (`src/object/object_primitive.c:11035`, `:11060`, `:11070`). The domain's `codeset` is at `ORC_DOMAIN_CODESET_OFFSET 12` and `collation_id` at `ORC_DOMAIN_COLLATION_ID_OFFSET 16` inside the attribute's packed domain (`src/base/object_representation.h:871`–`:872`). So a standalone reader must parse the domain substructure to know how to transcode string bytes; the record itself carries no codeset.

Note that `CHAR(N)` values are space-padded to the declared precision *before* compression, so the disk image preserves trailing-space semantics (`mr_writeval_char_internal`, `src/object/object_primitive.c:12192`–`:12210`, with the explanatory comment at `:12192`).

`BIT`/`VARBIT` use the parallel `or_put_varbit_internal` (`src/object/object_representation.c:754`), whose prefix is one byte of *bit* length when `bitlen < 0xFF`, else `0xFF` plus one big-endian 4-byte bit length (`:761`–`:772`), then `BITS_TO_BYTES(bitlen)` payload bytes, then 4-byte alignment. It has no compression path.

### 4.5 NUMERIC: a 3-byte header plus a variable-width magnitude

This encoding is not what older CUBRID documentation would suggest, and it is **identical on `develop`**, so it is a recent engine change rather than a feat-oos delta (verified by comparing `mr_data_writeval_numeric` at `src/object/object_primitive.c:8688` against `/home/vimkim/gh/cb/develop/src/object/object_primitive.c:8688` — byte-identical).

`mr_data_writeval_numeric` (`src/object/object_primitive.c:8688`) writes a 3-byte header (`NUMERIC_HEADER_SIZE 3`, `src/object/object_primitive.c:131`) followed by the low-order bytes of the internal numeric buffer:

```c
header[0] = disk_size | (negative ? NUMERIC_VALUE_SIGN_BIT_MASK : 0);   /* :8714 */
header[1] = precision | (scale < 0 ? NUMERIC_HEADER_SCALE_SIGN_BIT_MASK : 0);
header[2] = (scale < 0) ? -scale : scale;                                /* :8715-:8724 */
or_put_data (buf, header, NUMERIC_HEADER_SIZE);                          /* :8726 */
disk_size -= NUMERIC_HEADER_SIZE;
or_put_data (buf, (char *) numeric + (DB_NUMERIC_BUF_SIZE - disk_size), disk_size);  /* :8729 */
```

`NUMERIC_VALUE_SIGN_BIT_MASK` and `NUMERIC_HEADER_SCALE_SIGN_BIT_MASK` are both `0x80` (`src/query/numeric_opfunc.h:120`–`:121`), and `DB_NUMERIC_BUF_SIZE` is 17 (`src/compat/dbtype_def.h:647`).

So `header[0] & 0x7F` is the **total on-disk size including the 3-byte header**, and the magnitude is the trailing `disk_size - 3` bytes of a 17-byte big-endian-ish internal buffer. `mr_data_readval_numeric` (`src/object/object_primitive.c:8743`) confirms: it reads `size = OR_GET_BYTE (buf->ptr) & 0x7F` when the caller passes size -1 or 1 (`:8757`–`:8765`), then re-derives `size = header[0] & 0x7F` from the header proper (`:8796`), with explicit bounds guards at `:8760`, `:8790`, `:8798`.

A reimplementer should treat the trailing bytes as an unsigned big-endian integer, apply the sign from `header[0] & 0x80`, and divide by `10^scale` where scale comes from `header[2]` with its sign from `header[1] & 0x80`.

There is one write-path quirk worth knowing about, since it affects what is already on disk: auto-increment `NUMERIC` columns get their precision overridden to the user column's precision before write, specifically to avoid a byte-size mismatch against `_db_serial.current_val`'s `NUMERIC(38)` (`heap_attrinfo_transform_variable_to_disk`, `src/storage/heap_file.c:13115`–`:13119`, with the explanation at `:13109`–`:13114`).

---

## 5. Is a heap sector always exactly one class?

**Verdict: yes.** A sector is reserved to exactly one file for that file's entire lifetime and is never shared with another file; and a class has exactly one heap file. So caching a class's in-memory representation keyed by sector — built on the first click of any page in that sector — is sound. The caveats below are about what the cached attribution *means* for a given page, not about correctness of the sector→class mapping.

### 5.1 Sectors are 64 aligned pages

`#define DISK_SECTOR_NPAGES 64` (`src/storage/storage_common.h:109`), with the warning comment "Careful about changing this size. The whole file manager depends on this size" at `:108`. Sector *s* of a volume is exactly pages `64s` through `64s+63`: `SECTOR_FIRST_PAGEID(sid) ((sid) * DISK_SECTOR_NPAGES)`, `SECTOR_LAST_PAGEID`, `SECTOR_FROM_PAGEID(pageid) ((pageid) / DISK_SECTOR_NPAGES)` (`src/storage/storage_common.h:115`–`:117`). No sub-sector aliasing.

### 5.2 Reservation is whole-sector, exclusive, and released only on file destroy

`disk_reserve_sectors` (`src/storage/disk_manager.c:4290`) has exactly three call sites, all in the file manager: `file_create` (`src/storage/file_manager.c:3433`), `file_perm_expand` (`:4720`), and temp-file expansion (`:8739`). The on-disk allocator is a bitmap with one bit per sector and **no owner field** — `disk_stab_unit_reserve` scans 64-bit units and returns the `(volid, sectid)` pairs it claimed (`src/storage/disk_manager.c:3544`, claims at `:3590`–`:3594`). Because the bitmap has no owner, the only record of ownership is the file's own partial/full sector tables, which is exactly why there is no reverse map (2.3).

Every page a permanent file ever gets comes out of a sector already in that file's partial table. `file_perm_alloc` (`src/storage/file_manager.c:5188`) expands if `n_page_free == 0` (`:5224`, expansion at `:5227`), reads the file's own partial table (`:5238`), takes its first entry (`:5259`), and calls `file_partsect_alloc` (`:5268`), which derives the VPID arithmetically from the sector it owns:

```c
vpid_out->volid = partsect->vsid.volid;
vpid_out->pageid = SECTOR_FIRST_PAGEID (partsect->vsid.sectid) + offset_to_zero;
```

at `src/storage/file_manager.c:2852`, body `:2872`–`:2874`. There is no code path that produces a VPID outside the file's reserved sectors.

Deallocation never releases a sector. `file_perm_dealloc` (`src/storage/file_manager.c:6331`) clears the page bit and, if the sector was in the full table, moves it back into the *same file's* partial table (`:6459`–`:6461`). It never calls `disk_unreserve*`. Grepping the whole storage layer, `disk_unreserve_ordered_sectors` has exactly two call sites: `file_destroy` (`src/storage/file_manager.c:4345`, function at `:4143`) and rollback of a failed temp-file create (`:3905`). **A sector leaves a file only when the whole file is destroyed.**

### 5.3 One heap file per class

Server-side heap creation has one entry point, `xheap_create` → `heap_create_internal` (`src/storage/heap_file.c:5427`, `:4771`), and its callers are the schema manager's single instance heap (`src/object/schema_manager.c:16035`) and the three boot heaps with a NULL class OID (`src/transaction/boot_sr.c:4989`, `:4995`, `:5002`). The TRUNCATE-by-destroy path destroys and then creates (`heap_destroy_newly_created` at `src/object/schema_manager.c:16019`, then `heap_create` at `:16035`) — never two live heaps for one class.

Supporting evidence: the server keeps a single `class_oid → HFID` lock-free hash whose comment states "no collisions are expected when `heap_cache_class_info` is called" (`heap_cache_class_info`, `src/storage/heap_file.c:25534`, comment at `:25560`), and which asserts the file type is `FILE_HEAP` or `FILE_HEAP_REUSE_SLOTS` (`:25545`).

The sub-cases:

- **Reuse-slots** is a *substitute*, not an addition: `const FILE_TYPE file_type = reuse_oid ? FILE_HEAP_REUSE_SLOTS : FILE_HEAP;` (`src/storage/heap_file.c:4781`).
- **Partitioned classes** are N subclasses, each with its own class OID *and* its own HFID (`heap_get_partitions_from_subclasses`, `src/storage/heap_file.c:11615` region, per-subclass HFID from `heap_class_get_partition_info` at `:11560`). So N partitions give N heap files, N disjoint sector sets, N distinct class OIDs. Attribution stays exact; it just resolves to the partition rather than the parent, which is arguably the more useful answer for a volume map.
- **Multipage overflow** is a separate file with its own sectors: `heap_ovf_find_vfid` (`src/storage/heap_file.c:6081`) lazily creates a `FILE_MULTIPAGE_OBJECT_HEAP`, copying the heap's class OID into `des.heap_overflow.class_oid` (`:6131`–`:6132`) and calling `file_create_with_npages`, which reaches its own `disk_reserve_sectors`.
- **OOS** is likewise a separate numerable file: `oos_create_file_internal` calls `file_create (thread_p, FILE_OOS, ..., is_temp = false, is_numerable = true, &oos_vfid)` (`src/storage/oos_file.cpp:981`–`:982`) and its descriptor carries the class OID (`oos_create_file`, `src/storage/oos_file.cpp:1069`, fills at `:1074`–`:1075`). It does not share sectors with the heap.
- **Temporary heaps do not exist.** Query workspaces use `FILE_TEMP` and `FILE_QUERY_AREA` (`file_create_temp` at `src/storage/file_manager.c:3224`, `file_create_query_area` at `:3253`), both routed through `file_create_temp_internal` with `is_temp = true`. Temp-purpose sectors are reserved from temporary-purpose volumes only: `disk_reserve_from_cache_vols` skips any volume whose purpose differs (`src/storage/disk_manager.c:4642`–`:4646`). **An offline reader should check `DISK_VOLUME_HEADER.purpose` first — a permanent-purpose volume contains only permanent files.**
- **Reused heaps** rewrite `class_oid` everywhere. `heap_reuse` rewrites the header's and every chain's `class_oid` and resets `max_mvccid`/`flags`, and `file_tracker_item_reuse_heap` rewrites the file descriptor (`src/storage/file_manager.c:10457`–`:10467`). Descriptor and pages therefore never disagree; the only stale window is a mark-deleted heap still naming the dropped class.
- **Files can span volumes.** `disk_reserve_sectors` loops over volumes (`src/storage/disk_manager.c:4345`–`:4353`), and `file_create` remembers `volid_last_expand` (`src/storage/file_manager.c:3443`). For heaps the file header page is forced onto the first volume (`:3458`, `:3463`) and `heap_create_internal` asserts the heap header page is on `hfid->vfid.volid` (`src/storage/heap_file.c:4854`–`:4860`), but **data pages of one class may sit in several permanent volumes**. A per-sector cache key must therefore be `(volid, sectid)`, not `sectid`.

### 5.4 Mixed page kinds inside one heap sector — same file only

Sectors do contain heterogeneous pages, but always of the same file. Two places prove it:

- At creation, when the reserved sectors do not fit in the header page's partial table, `file_create` allocates extra `PAGE_FTAB` pages *out of the sectors it just reserved*: the page id is computed as `SECTOR_FIRST_PAGEID (partsect_ftab->vsid.sectid)` (`src/storage/file_manager.c:3692`–`:3693`), the header page's own page is skipped (`:3712`–`:3717`), the bit is set in the same bitmap (`file_partsect_set_bit` at `:3721`), and `fhead->n_page_ftab++` (`:3744`). The file header page's own bit is set at `:3750`–`:3752`.
- At runtime, table pages are allocated with `FILE_ALLOC_TABLE_PAGE` through the same `file_perm_alloc` and counted by `file_header_alloc` as `fhead->n_page_ftab++` (`src/storage/file_manager.c:1107`), then collected back at destroy time by `file_table_collect_ftab_pages` (`:7156`).

So a heap file's sector legitimately holds:

| Page kind | `PAGE_TYPE` | Slotted? | Class OID on the page? |
|---|---|---|---|
| File header page (`hfid.vfid.fileid`) | `PAGE_FTAB` | No | Yes, in `FILE_HEAP_DES.class_oid` at user offset 40 (`src/storage/file_manager.c:3514`, `src/storage/file_manager.h:83`) |
| Extended file-table pages | `PAGE_FTAB` | No | **No** — bare `FILE_EXTENSIBLE_DATA` + items (`src/storage/file_manager.c:231`) |
| Heap header page (`hfid.hpgid`) | `PAGE_HEAP` | Yes | Yes, slot 0 = `HEAP_HDR_STATS`, `class_oid`@0 |
| Bestspace pages | `PAGE_HEAP` | Yes | Yes, slot 0 = `HEAP_CHAIN` with the `BESTSPACE` flag; slot 1 is the entry array (`src/storage/heap_file.c:3950`, `:4014`–`:4021`) |
| Heap data pages | `PAGE_HEAP` | Yes | Yes, slot 0 = `HEAP_CHAIN` (`src/storage/heap_file.c:2866`) |
| Reserved-but-unallocated (bitmap bit 0) | garbage | — | **No** — never initialized |

A reader that attributes by sector membership gets the *right class* for every one of these, including the `PAGE_FTAB` pages, because they genuinely are owned by that heap file. The attribution is correct in the "owned by this class's heap file" sense while being misleading in the "holds rows of this table" sense. All four categories are cheaply distinguishable offline — file-header page by VPID equality with `hfid.vfid.fileid`, heap metadata by slot-0 record length (1160) or the `BESTSPACE` flag, data pages by slot-0 length 40 with that flag clear, and unallocated pages by the partial-sector bitmap bit — so a volume map should label them rather than lump them.

Also watch `HEAP_PAGE_FLAG_NOT_IN_HEAP` (`src/storage/heap_file.c:235`): a page allocated but not yet linked into the heap (`heap_page_is_not_in_heap`, `:2814`).

### 5.5 The first sector of a volume is special

Pages `0 .. sys_lastpage` are system pages and their covering sectors are pre-marked reserved at format time, belonging to no file. Page 0 is the `DISK_VOLUME_HEADER` (`DISK_VOLHEADER_PAGE 0`, `src/storage/disk_manager.h:35`; written as a raw struct, not a slotted page, at `disk_format`, `src/storage/disk_manager.c:596`, `:599`). The sector table follows: `stab_first_page = DISK_VOLHEADER_PAGE + 1`, `stab_npages = CEIL_PTVDIV (nsect_max, DISK_STAB_PAGE_BIT_COUNT)`, `sys_lastpage = stab_first_page + stab_npages - 1` (`src/storage/disk_manager.c:3168`–`:3170`). `disk_stab_init` (`:4909`) computes `nsects_sys = SECTOR_FROM_PAGEID (volheader->sys_lastpage) + 1` (`:4911`) and marks those leading bits via `disk_stab_set_bits_contiguous` (`:4959`–`:4960`), page type `PAGE_VOLBITMAP` at `:4932`. Leftover pages between `sys_lastpage+1` and the end of that sector are reserved but in no file's table.

---

## 6. Practical recipe

### 6.1 Bootstrap, once per database

1. Read volume 0, page 0. The user area begins at file offset 32. Validate the `magic[25]` field at user offset 0 against `"CUBRID/Volume"` (`CUBRID_MAGIC_DATABASE_VOLUME`, `src/storage/storage_common.h:405`; `CUBRID_MAGIC_MAX_LENGTH 25` at `:403`) and read `iopagesize`. `struct disk_volume_header` is at `src/storage/disk_manager.c:74`, with `magic` at `:78`, `iopagesize` at `:79`, `volid` at `:80`, `boot_hfid` at `:97`. Note the source's own caveat that `iopagesize` "was only added for checking purposes; the actual value is stored on the log" (`:79`).
2. Read `boot_hfid` and walk that heap to its first user record. That record is a raw `struct boot_dbparm` image — `recdes.data = (char *) dbparm; recdes.length = DB_SIZEOF (*dbparm)` then `heap_first` (`boot_get_db_parm`, `src/transaction/boot_sr.c:300`, `heap_first` at `:310`). **This one struct is native-endian with native padding**, unlike everything else discussed here. From it take `rootclass_hfid` (`:126`), `ctid` (`:128`), and `rootclass_oid` (`:132`).
3. The record's OID is never persisted, so the reader must replicate "first non-metadata slot, following the heap page chain" rather than looking one up.
4. Follow `vhdr->next_volid` / `next_vol_fullname` to enumerate the remaining permanent volumes (`disk_get_link`, used from `boot_find_rest_permanent_volumes`, `src/transaction/boot_sr.c:1065`, `:1098`).

Also skip the empty extendible hashes: `ctid.xhid` (3.1) and `boot_dbparm.classname_table` (created at `src/transaction/boot_sr.c:5027`, never searched).

### 6.2 From `(volume file, page id, slot id)` to values

1. **Read the page.** Seek to `page_id * page_size`, take the user area at `+32`, and confirm `prv.pageid`/`prv.volid` at prologue offsets 8 and 12 match what you asked for (section 1.1).
2. **Read the slot.** Slot *n*'s 4-byte descriptor is at `user_area_end - 4*(n+1)`; unpack `offset_to_record:14, record_length:14, record_type:4` (section 1.1).
3. **Handle the record type.** `REC_HOME`/`REC_NEWHOME` → proceed. `REC_RELOCATION` → follow the forwarding OID to its `REC_NEWHOME`. `REC_BIGONE` → the payload is in the class's multipage overflow file; follow the `OVERFLOW_FIRST_PART`/`OVERFLOW_REST_PART` chain (`src/storage/overflow_file.h:37`) and reassemble before continuing. `REC_MARKDELETED`/`REC_DELETED_WILL_REUSE` → no record.
4. **Get the class OID.** Read slot 0 of the same page. Length 1160 → header page, `HEAP_HDR_STATS`; length 40 → data page, `HEAP_CHAIN`. Either way `class_oid` is at offset 0. A NULL OID means the root class (section 2.4). This is where a per-sector cache short-circuits everything below.
5. **Get the reprid.** `repid = OR_GET_INT(record + 0) & 0x00FFFFFF`; `mvcc_flags = (OR_GET_INT(record + 0) >> 24) & 0x07`; `header_size = mvcc_header_size_lookup[mvcc_flags]`; `offset_size` from bits 29–30; `has_bound_bits` from bit 31 (section 1.2, 1.3).
6. **Load the representation.** Read the class object's heap record at exactly the `class_oid` (it is a physical `(volid, pageid, slotid)` address, not a key). Compare the record's reprid to the target: equal → parse the current representation as in `or_get_current_representation` (3.5); different → search the packed old-representation set at variable attribute `ORC_REPRESENTATIONS_INDEX = 2`. Result needed: `n_fixed`, `n_variable`, `fixed_length`, and per attribute `{id, name, type, domain (precision, scale, codeset, collation), is_fixed, location, position}`.
7. **Cache it.** Key by `(volid, sectid)` — files can span volumes (5.3) — and optionally also by class OID so partitions and multiple sectors of one heap share the parse.
8. **Decode fixed attributes.** `fixed_base = header_size + OR_VAR_TABLE_SIZE_INTERNAL(n_variable, offset_size)`. For each fixed attribute: if `has_bound_bits` and the bit at `position` in the array at `fixed_base + fixed_length` is clear, the value is NULL; otherwise decode `tp_domain_disk_size(domain)` bytes at `fixed_base + location` per section 4.3.
9. **Decode variable attributes.** Read offset-table entry `location` and entry `location + 1`; mask both with `~0x3` (feat-oos) to get true offsets, keeping the raw entry to test `OR_VAR_BIT_OOS`. A zero span means NULL. Otherwise the value starts at `header_size + masked_entry`. If the OOS bit is set, the 16 bytes there are `[OID (8) | full length (8 bigint)]` and the real value lives in the class's `FILE_OOS` file (section 7). Otherwise decode by type per sections 4.4 and 4.5.
10. **Transcode strings.** Use the domain's `codeset`/`collation_id` (offsets 12 and 16 in the packed domain, `src/base/object_representation.h:871`–`:872`) to convert to UTF-8 for display. The record carries no codeset of its own.

Steps 6 and 7 are the whole reason the per-sector cache matters: everything before step 6 is a couple of page reads, and everything after it is pure byte arithmetic.

The system catalog is not needed for any of this. Reach for `catalog_get_representation` (section 3.4) only if you want the optimizer's statistics.

---

## 7. feat-oos deltas observed

These are the differences from `develop` (`e6ed61e87`) that affect how normal records are read.

**1. Two flag bits stolen from every variable-offset-table entry.** `OR_VAR_BIT_OOS 0x1`, `OR_VAR_BIT_LAST_ELEMENT 0x2`, `OR_VAR_FLAG_MASK 0x3`, and `OR_GET_VAR_OFFSET` at `src/base/object_representation.h:445`–`:456` are new, and `OR_VAR_TABLE_ELEMENT_OFFSET_INTERNAL` now masks every read (`:476`). On `develop` the same macro returns the raw value (`/home/vimkim/gh/cb/develop/src/base/object_representation.h:447`–`:452`). This is format-compatible in the sense that the bits were always zero (variable values are `INT_ALIGNMENT`-padded, 4.4), but a reader written against `develop` will produce wrong offsets on OOS records and a reader written against feat-oos is safe on both.

**2. A fourth record flag.** `OR_RECORD_FLAG_HAS_OOS 0x08` is new (`src/base/object_representation_constants.h:174`), as is the split between `OR_RECORD_FLAG_MASK 0x1f` (all 5 flag bits) and `OR_RECORD_MVCC_FLAG_MASK 0x07` (the 3 bits that drive header size) at `:159`–`:160`. On `develop` there is a single `OR_MVCC_FLAG_MASK 0x1f` and no OOS bit (`/home/vimkim/gh/cb/develop/src/base/object_representation_constants.h:160`). Header size is unaffected either way, since feat-oos masks to 3 bits before the lookup.

**3. The 16-byte OOS inline stub.** `OR_OOS_INLINE_SIZE = OR_OID_SIZE + OR_BIGINT_SIZE` = 16 (`src/base/object_representation.h:459`). Layout is `[OID (8) | full_length (8, bigint)]`, documented and parsed by `heap_oos_parse_inline_ref` (`src/storage/heap_oos.cpp:430`, comment at `:421`, reads at `:449`–`:450`, validation at `:453`). Resolution is `heap_attrvalue_read_oos_inline` (`src/storage/heap_file.c:10450`), reached from `heap_attrvalue_point_variable` when `OR_IS_OOS(entry)` (`:10544`–`:10547`).

**4. `oos_vfid` inserted into `HEAP_HDR_STATS` at offset 32.** Declared at `src/storage/heap_file.c:208`, which pushes `unfill_space` from 32 to 40. `sizeof(HEAP_HDR_STATS)` stays 1160 because the struct has trailing reserved ints (`:228`–`:230`). volmap already reads `oos_vfid` at `+32` and `unfill_space` at `+40` (`/home/vimkim/temp/volmap/src/format/heap.rs:303`, `:296`).

**Stale comment worth reporting upstream:** `heap_page_is_not_in_heap` says "`HEAP_HDR_STATS::unfill_space` overlaps `HEAP_CHAIN::flags` at the same offset" (`src/storage/heap_file.c:2827`–`:2828`). That was true before `oos_vfid` existed — on `develop` `unfill_space` sits at offset 32, the same as `HEAP_CHAIN::flags`. On feat-oos `oos_vfid` occupies 32 and `unfill_space` moved to 40, so the two no longer overlap. The code is still correct because it branches on record length, not on the overlap, but the comment now misdescribes the layout.

**5. New file type `FILE_OOS` with `FILE_OOS_DES { HFID hfid; OID class_oid; }`** (`src/storage/file_manager.h:53`, `:99`). It is created numerable and permanent with its own reserved sectors (`src/storage/oos_file.cpp:981`–`:982`), so it does not weaken the one-sector-one-file invariant, and its own sectors carry their own class attribution via the descriptor.

**6. OOS and `REC_BIGONE` are mutually exclusive by construction.** `heap_attrinfo_transform_to_disk_internal` rejects the combination with a user-visible error before writing anything (`src/storage/heap_file.c:13300`–`:13305`, with the reasoning at `:13295`–`:13299`). A reader will therefore never see an OOS stub inside a multipage overflow payload.

**7. Attribute-level OOS storage hints.** `SM_ATTFLAG_OOS_FORCE_OUTLINE` / `SM_ATTFLAG_OOS_PREFER_INLINE` are read from the attribute flags word into `att->oos_storage` (`src/base/object_representation_sr.c:2523`–`:2536`). Demotion policy is in `heap_attrinfo_determine_disk_layout` (`src/storage/heap_file.c:12310`): forced columns go out first (`:12331`–`:12342`), then the largest remaining variable columns are pushed out one at a time until the inline record fits `DB_PAGESIZE / 4` (`:12350`–`:12395`, described as "PG TOAST style" at `:12348`–`:12349`). This is write-path only and does not change how a reader parses a record, but it explains why a wide `develop` row may be `REC_BIGONE` while the same row on feat-oos is a small in-row record with stubs.

Sections 1 through 6 above describe the feat-oos format. The only place a reader must branch on branch identity is item 1, and masking is the safe superset.

---

## What could not be verified

- **`sizeof(HEAP_HDR_STATS) = 1160` and `sizeof(FILE_HEADER) = 216`** are not asserted anywhere in the CUBRID tree. The 1160 figure and the field offsets in 2.1 agree between the struct declaration and volmap's decoder reading real volumes, which is strong; `FILE_HEADER`'s total size was not independently confirmed (only the descriptor's offset 40, which volmap does confirm). There is a runtime `assert (FILE_DESCRIPTORS_SIZE == sizeof (FILE_DESCRIPTORS))` at `src/storage/file_manager.c:863` and a `static_assert (sizeof (bestspace_entry) == 8)` at `src/storage/bestspace.hpp:62`, but nothing pins the two heap structs or `FILE_HEADER`.
- **Byte offsets inside `struct boot_dbparm` and `DISK_VOLUME_HEADER`** were not derived here. Both are native C structs with no offset macros, so a reader must compute them for its own target ABI (LP64 x86-64 for this project) rather than trust a table. The field *order* is cited (`src/transaction/boot_sr.c:121` region, `src/storage/disk_manager.c:74` region); the offsets are not.
- **Whether a packed default value's `val_length` is ever a non-multiple of 8** in practice. It is established that the catalog writer inserts no padding (`src/storage/system_catalog.c:1248`) and that `val_length` is a raw remainder (`src/base/object_representation_sr.c:1413`); no audit was done of every `or_put_value` path to see whether upstream packing happens to align anyway. Treat the cursor as unaligned.
- **The set/multiset/sequence element encoding** was not worked through. The `OR_SET_*` header macros are at `src/base/object_representation.h:671`–`:720` and `heap_attrvalue_point_variable` computes a real span for these types (`src/storage/heap_file.c:10551`–`:10559`), but the element-level layout, domain tagging, and set bound bits are a separate task.
- **The `ENUMERATION` type's on-disk value** is a 2-byte index (`src/object/object_primitive.c:1664`) into the domain's enumeration substructure at `ORC_DOMAIN_ENUMERATION_INDEX = 1` (`src/base/object_representation.h:879`). The substructure's own encoding was not decoded.
- **The `BTREE_STATS.pkeys_size` problem** (3.4) means catalog statistics cannot be fully decoded without a B-tree root-page parser. No attempt was made to write one.
- **Log and recovery.** The catalog and heap are fully WAL-logged, so a crashed image needs log replay to be self-consistent. The log format was not examined, and neither were the `RVCT_*` / `RVHF_*` recovery records.
- **`heap_hfid_table` collision impossibility.** The "no collisions are expected" claim (`src/storage/heap_file.c:25559`) is a comment, not an enforced invariant; `lf_hash` was not audited to prove two live HFIDs for one class OID are structurally impossible.
- **ALTER TABLE exhaustively.** Only the TRUNCATE-by-destroy path in `schema_manager.c` was found creating a heap, and `xheap_create` has only the three caller sites listed in 5.3, which strongly implies no ALTER variant creates a second heap — but `do_alter*` in the parser and executor was not read.
