# Pinned `feat/oos` disk-format contract

## Scope and authority

This inventory describes the persistent bytes that a read-only external inspector must understand for the CUBRID worktree `/home/vimkim/gh/cb/feat-oos` at exactly commit `e1e651debf6cc100172bde96603b17424f9c135a`. The primary authority is that commit's source. ABI offsets below were independently measured from its generated debug objects under `build_preset_debug_gcc` with GDB `ptype /o`. The generated `demodb` fixture and recovered `volmap-standalone` are corroboration only.

The pinned profile is Linux x86-64, ELF `LSB`, GCC ABI, 16 KiB I/O pages. It is **not** a generally portable CUBRID format specification. Several persistent structures are raw C/C++ object representations, including padding and compiler bit-fields. A parser for this profile should decode explicit little-endian offsets into safe values; it should not reproduce the structs with Rust `repr(C)`, borrow unaligned fields, or infer offsets from its own compiler.

Primary scalar widths are `VOLID`/slot/length = signed 16-bit, `PAGEID`/`SECTID`/`FILEID` = signed 32-bit, and a sector is 64 pages ([`src/storage/storage_common.h:70-117`](/home/vimkim/gh/cb/feat-oos/src/storage/storage_common.h)). The default physical page is 16,384 bytes ([`src/storage/storage_common.h:89-101`](/home/vimkim/gh/cb/feat-oos/src/storage/storage_common.h)).

## Two byte-order domains

This distinction is the most important implementation fact.

1. **Raw native-layout metadata** is copied or addressed as structs and is little-endian with the pinned x86-64/GCC padding: I/O-page prefix, volume header, sector bitmap words, file headers, extensible tables, file-tracker items, slotted-page header and slots, heap/OOS statistics, and OOS chunk headers. For example, the OOS writer uses `memcpy` for `OOS_RECORD_HEADER` ([`src/storage/oos_file.cpp:1117-1139`](/home/vimkim/gh/cb/feat-oos/src/storage/oos_file.cpp)), and the volume source explicitly warns not to use `sizeof` because its string tail is variable ([`src/storage/disk_manager.c:73-111`](/home/vimkim/gh/cb/feat-oos/src/storage/disk_manager.c)).
2. **Object-representation (`OR_*`) bytes** use network/big-endian numeric encoding: `OR_PUT/GET_SHORT`, `INT`, and `INT64` use `htons`/`htonl`/byte swapping ([`src/base/object_representation.h:92-138`](/home/vimkim/gh/cb/feat-oos/src/base/object_representation.h)). Heap record representation words, variable-offset tables, and the 16-byte inline OOS reference are in this domain.

Consequently, an OID inside an OOS chunk header is native little-endian `{pageid:i32, slotid:i16, volid:i16}`, while the same logical OID in a heap inline OOS reference is big-endian at offsets `0/4/6` ([`src/base/object_representation.h:272-286`](/home/vimkim/gh/cb/feat-oos/src/base/object_representation.h), [`src/base/object_representation_constants.h:67-70`](/home/vimkim/gh/cb/feat-oos/src/base/object_representation_constants.h)).

## Database and volume discovery

### `databases.txt`

The database module names the environment suffix `DATABASES` and file `databases.txt` ([`src/base/databases_file.h:30-40`](/home/vimkim/gh/cb/feat-oos/src/base/databases_file.h)). `envvar_get` prefixes that suffix with `CUBRID_`, so the actual variable is `CUBRID_DATABASES` ([`src/base/environment_variable.c:177-191`](/home/vimkim/gh/cb/feat-oos/src/base/environment_variable.c)). If it is non-empty, the file is `$CUBRID_DATABASES/databases.txt`; otherwise the engine's directory-file reader uses local `./databases.txt` ([`src/base/databases_file.c:214-243`](/home/vimkim/gh/cb/feat-oos/src/base/databases_file.c)). The recovered utility's additional `$CUBRID/databases/databases.txt` fallback is an application convenience, not engine directory-file behavior.

Nonblank, non-`#` lines are whitespace-tokenized as:

```text
database-name  volume-directory  primary-host  log-directory  [lob-path]
```

The first four tokens are required; the LOB token is tolerated as absent for backward compatibility ([`src/base/databases_file.c:391-479`](/home/vimkim/gh/cb/feat-oos/src/base/databases_file.c)). There is no quoting in this parser, so whitespace cannot occur inside a path. The database's first-volume path is `volume-directory/database-name`; its volume-info path is that path plus `_vinf`.

### `_vinf`

`_vinf` is the suffix defined at [`src/storage/file_io.h:81-94`](/home/vimkim/gh/cb/feat-oos/src/storage/file_io.h). Each entry is text written as `%4d %s\n`: signed decimal volume ID, whitespace, path ([`src/transaction/log_page_buffer.c:4683-4717`](/home/vimkim/gh/cb/feat-oos/src/transaction/log_page_buffer.c), [`src/transaction/log_page_buffer.c:4800-4835`](/home/vimkim/gh/cb/feat-oos/src/transaction/log_page_buffer.c)). The reader uses `%d %PATH_MAXs`, stops at the first unparsable pair, and rejects decreasing IDs after the first item ([`src/transaction/log_page_buffer.c:4852-4930`](/home/vimkim/gh/cb/feat-oos/src/transaction/log_page_buffer.c)). Negative IDs name log metadata; persistent data volumes start at 0. An offline volume inspector should retain only existing data-volume entries with nonnegative IDs, cross-checking each header's `volid`.

The header's `next_volid` and next-volume path are an independent chain from which `_vinf` can be recreated ([`src/transaction/log_page_buffer.c:4721-4777`](/home/vimkim/gh/cb/feat-oos/src/transaction/log_page_buffer.c)). Treat disagreement between `_vinf` and that chain as corruption/staleness, not permission to open arbitrary unvalidated paths.

## Physical I/O page envelope

Physical page `p` begins at file offset `p * IO_PAGESIZE` ([`src/storage/file_io.c:198-207`](/home/vimkim/gh/cb/feat-oos/src/storage/file_io.c), [`src/storage/file_io.c:3922-3957`](/home/vimkim/gh/cb/feat-oos/src/storage/file_io.c)). For this profile:

```text
physical bytes 0..31       FILEIO_PAGE_RESERVED (plaintext)
physical bytes 32..16375   16,344-byte database/user page
physical bytes 16376..16383 FILEIO_PAGE_WATERMARK (plaintext)
```

The 16,344-byte user-page size is 16,384 minus the 32-byte prefix and 8-byte watermark ([`src/storage/storage_common.c:43-48`](/home/vimkim/gh/cb/feat-oos/src/storage/storage_common.c)). GDB confirms the prefix layout declared at [`src/storage/file_io.h:164-193`](/home/vimkim/gh/cb/feat-oos/src/storage/file_io.h):

| Physical offset | Size | Field |
|---:|---:|---|
| 0 | 8 | raw GCC `LOG_LSA` bit-field (`pageid:48`, `offset:16`) |
| 8 | 4 | `pageid` |
| 12 | 2 | `volid` |
| 14 | 1 | physical `PAGE_TYPE` |
| 15 | 1 | page flags |
| 16 | 4 | reserved 1 |
| 20 | 4 | reserved 2 |
| 24 | 8 | TDE nonce |
| 16376 | 8 | duplicate `LOG_LSA` watermark |

The only implemented generic page-corruption check is equality of the leading and trailing LSAs ([`src/storage/file_io.h:195-236`](/home/vimkim/gh/cb/feat-oos/src/storage/file_io.h), [`src/storage/file_io.c:11918-11931`](/home/vimkim/gh/cb/feat-oos/src/storage/file_io.c)). `fileio_set_page_checksum` is declared but has no implementation in this commit; there is no checksum field to validate. An inspector should additionally require prefix `(volid,pageid)` to equal the containing volume/page and reject unknown `ptype` values.

Physical page-type ordinals are ([`src/storage/storage_common.h:148-167`](/home/vimkim/gh/cb/feat-oos/src/storage/storage_common.h)):

| Value | Type | Value | Type |
|---:|---|---:|---|
| 0 | UNKNOWN | 8 | OOS |
| 1 | FTAB | 9 | AREA |
| 2 | HEAP | 10 | CATALOG |
| 3 | VOLHEADER | 11 | BTREE |
| 4 | VOLBITMAP | 12 | LOG (unused) |
| 5 | QRESULT | 13 | DROPPED_FILES |
| 6 | EHASH | 14 | VACUUM_DATA |
| 7 | OVERFLOW | | |

### TDE limitation

Page flag bit `0x1` means AES and `0x2` means ARIA; both simultaneously are invalid ([`src/storage/file_io.h:62-66`](/home/vimkim/gh/cb/feat-oos/src/storage/file_io.h), [`src/storage/page_buffer.c:5080-5149`](/home/vimkim/gh/cb/feat-oos/src/storage/page_buffer.c)). When set, exactly the 16,344-byte user region is encrypted; the prefix and watermark are copied in plaintext ([`src/storage/tde.h:42-50`](/home/vimkim/gh/cb/feat-oos/src/storage/tde.h), [`src/storage/tde.c:913-1002`](/home/vimkim/gh/cb/feat-oos/src/storage/tde.c)). A strict standalone inspector without CUBRID's loaded data key can report identity/type/algorithm/nonce and LSA integrity, but must label all user-page metadata unavailable rather than parse ciphertext.

## Volume header and sector allocation

Every volume's page 0 has physical type `PAGE_VOLHEADER`; its 16,344-byte user region starts with raw `DISK_VOLUME_HEADER`. The source writes magic `CUBRID/Volume`, page size, identifiers and allocation fields directly ([`src/storage/disk_manager.c:574-665`](/home/vimkim/gh/cb/feat-oos/src/storage/disk_manager.c)); magic length is 25 ([`src/storage/storage_common.h:403-405`](/home/vimkim/gh/cb/feat-oos/src/storage/storage_common.h)). GDB gives this pinned layout:

| User offset | Field | User offset | Field |
|---:|---|---:|---|
| 0 | `magic[25]` | 56 | `stab_npages:i32` |
| 25 | 1-byte ABI hole | 60 | `stab_first_page:i32` |
| 26 | `iopagesize:i16` | 64 | `sys_lastpage:i32` |
| 28 | `volid:i16` | 68 | alignment dummy `i32` |
| 30 | charset `i8` | 72 | database creation `i64` |
| 31 | alignment dummy `i8` | 80 | volume creation `i64` |
| 32 | purpose enum `i32` | 88 | checkpoint `LOG_LSA` (8) |
| 36 | volume type enum `i32` | 96 | boot `HFID` (12) |
| 40 | pages/sector `i32` | 108..123 | four reserved `i32` |
| 44 | total sectors `i32` | 124 | `next_volid:i16` |
| 48 | maximum sectors `i32` | 126/128/130 | three string offsets `i16` |
| 52 | allocation hint `i32` | 132 | `var_fields[]` |

Purpose ordinals are permanent=0, temporary=1; type ordinals are permanent=0, temporary=1 ([`src/compat/dbtype_def.h:197-209`](/home/vimkim/gh/cb/feat-oos/src/compat/dbtype_def.h)). The three string offsets are relative to `var_fields` at user offset 132, not to the header start. They address NUL-terminated current-volume path, next-volume path, and remarks; used bytes end after the remarks NUL ([`src/storage/disk_manager.c:5451-5503`](/home/vimkim/gh/cb/feat-oos/src/storage/disk_manager.c)). The C struct's padded `sizeof` is 136 but is not the serialized header length.

Strict invariants are: magic and `iopagesize=16384`; header `volid` agrees with `_vinf` and prefix; `sect_npgs=64`; positive, 64-rounded `nsect_total <= nsect_max`; `stab_first_page=1`; `stab_npages=ceil(nsect_max/(DB_PAGESIZE*8))`; `sys_lastpage=stab_first_page+stab_npages-1`; valid purpose/type pair; monotone in-page string offsets with terminating NULs; file length is a 16,384-byte multiple and accommodates `nsect_total*64` pages. These mirror [`src/storage/disk_manager.c:3165-3203`](/home/vimkim/gh/cb/feat-oos/src/storage/disk_manager.c).

Sector allocation pages begin at page 1 and have type `PAGE_VOLBITMAP`. Their user regions are raw arrays of little-endian `u64`; one bit per sector, least-significant bit first. For sector `s`: allocation page offset is `s/(16344*8)`, word is `(s % (16344*8))/64`, bit is `s%64` ([`src/storage/disk_manager.c:213-263`](/home/vimkim/gh/cb/feat-oos/src/storage/disk_manager.c)). Set means reserved. Initial formatting zeroes these pages and reserves the system sectors covering the volume header and bitmap pages ([`src/storage/disk_manager.c:4901-4989`](/home/vimkim/gh/cb/feat-oos/src/storage/disk_manager.c)). Bits outside `nsect_total` are not usable sectors.

## Logical files, ownership, and page allocation

### File types and identifiers

`FILE_TYPE` ordinals are: tracker 0, heap 1, heap-reuse-slots 2, multipage-object-heap 3, btree 4, btree-overflow-key 5, extensible-hash 6, hash-directory 7, catalog 8, dropped-files 9, vacuum-data 10, query-area 11, temp 12, OOS 13, unknown 14 ([`src/storage/file_manager.h:38-56`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.h)). A `VFID` is raw `{fileid:i32 @0, volid:i16 @4, pad @6}` and its header `VPID` is `(volid, pageid=fileid)` ([`src/storage/file_manager.c:195-211`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.c)). File-header pages have physical type `PAGE_FTAB`.

The authoritative inventory root is the file tracker. Bootstrap is:

1. Volume 0 header `boot_hfid` identifies the boot heap.
2. The engine retrieves that heap's first user record as raw `BOOT_DB_PARM` ([`src/transaction/boot_sr.c:290-320`](/home/vimkim/gh/cb/feat-oos/src/transaction/boot_sr.c), [`src/transaction/boot_sr.c:2309-2389`](/home/vimkim/gh/cb/feat-oos/src/transaction/boot_sr.c)). On this ABI it is 136 bytes and starts with `trk_vfid` at offset 0; the source definition is [`src/transaction/boot_sr.c:121-141`](/home/vimkim/gh/cb/feat-oos/src/transaction/boot_sr.c).
3. Decode the tracker file header and follow its `vpid_sticky_first` to a `PAGE_FTAB` extensible-data chain of 16-byte tracker items. Each item is `{fileid:i32, volid:i16, type:i16, metadata[8]}` ([`src/storage/file_manager.c:492-523`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.c), [`src/storage/file_manager.c:9877-10027`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.c)). The tracker itself is not registered in itself.

A damage-tolerant scan may corroborate/recover file headers by testing every allocated `PAGE_FTAB` page for `self == containing VPID` and all header invariants. That is the recovered binary's approach, not an alternate authoritative root.

### File header (raw, 216 bytes)

GDB confirms the source layout at [`src/storage/file_manager.c:85-167`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.c):

| Offset | Field | Offset | Field |
|---:|---|---:|---|
| 0 | creation time `i64` | 124/128/132/136 | sector total/partial/full/empty `i32` |
| 8 | self `VFID` (8) | 140 | file type `i32` |
| 16 | tablespace (24) | 144 | flags `i32` |
| 40 | type-specific descriptor union (64) | 148 | last-expand volume `i16` |
| 104/108/112/116/120 | page total/user/ftab/free/marked-delete `i32` | 150/152/154 | partial/full/user-table offsets `i16` |
| 156 | sticky-first `VPID` | 164 | last-temp-allocation `VPID` |
| 172 | temp cursor `i32` | 176 | last user-table `VPID` |
| 184 | find-nth cache `VPID` | 192 | find-nth index `i32` |
| 196..211 | four reserved `i32` | 212..215 | ABI tail padding |

Flags are numerable `0x1`, temporary `0x2`, AES `0x4`, ARIA `0x8` ([`src/storage/file_manager.c:169-179`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.c)). OOS files in this commit are permanent and explicitly created numerable ([`src/storage/oos_file.cpp:968-983`](/home/vimkim/gh/cb/feat-oos/src/storage/oos_file.cpp)). The OOS descriptor in the 64-byte union is raw `{owner HFID (12), owner class OID (8)}` at descriptor offsets 0 and 12, giving direct logical ownership ([`src/storage/file_manager.h:97-149`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.h), [`src/storage/oos_file.cpp:1068-1077`](/home/vimkim/gh/cb/feat-oos/src/storage/oos_file.cpp)).

File counters must be nonnegative with `free + user + ftab = total`, `marked_delete <= user`, `partial + full = sector_total`, and `empty <= partial`; table item counts must agree with their counters and no sector may occur twice ([`src/storage/file_manager.c:928-1044`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.c)).

### Extensible tables and sector ownership

Every component begins with a raw 16-byte header: `next VPID @0` (8), `max_size:i16 @8`, `item_size:i16 @10`, `n_items:i16 @12`, two pad bytes. Items start at offset 16; used bytes are `16 + n_items*item_size` ([`src/storage/file_manager.c:228-240`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.c), [`src/storage/file_manager.c:1488-1613`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.c)). The first components live at the file-header offsets; continuation components occupy user offset 0 of linked `PAGE_FTAB` pages. Permanent numerable headers divide remaining space 1/32 partial, 1/32 full, remainder ordered user VPIDs; permanent nonnumerable files divide it half partial/half full ([`src/storage/file_manager.c:3567-3645`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.c)).

Partial-sector items are 16 bytes: raw `VSID` (sector `i32`, volume `i16`, pad) plus `u64 page_bitmap`. Bit `n` means page `sector*64+n` is allocated; bit 0 is the least-significant bit. Full-sector items are 8-byte raw `VSID` values and imply all 64 pages ([`src/storage/file_manager.h:161-177`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.h), [`src/storage/file_manager.c:2780-2874`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.c), [`src/storage/file_manager.c:12559-12603`](/home/vimkim/gh/cb/feat-oos/src/storage/file_manager.c)). Numerable user-page items are 8-byte raw VPIDs in allocation order; they are an index, not additional ownership.

Thus sector ownership is tracker item -> file header -> partial/full tables. A sector-level volume bitmap says only reserved/unreserved; the file tables say which file reserved it and which pages are live. A strict inspector must bound all component arithmetic inside 16,344 bytes, require positive recognized item sizes (partial 16/full 8/user 8/tracker 16), validate next VPIDs and `PAGE_FTAB`, detect arbitrary cycles with a visited set, ensure referenced sectors are in range and reserved, and report duplicate file ownership as corruption.

## Generic slotted pages

Heap, OOS, and several other page types store raw `SPAGE_HEADER` at user offset 0 and raw slot words from the end of the 16,344-byte user region backward. GDB confirms a 32-byte header and 4-byte slot from [`src/storage/slotted_page.h:56-91`](/home/vimkim/gh/cb/feat-oos/src/storage/slotted_page.h):

| Header offset | Field |
|---:|---|
| 0/2 | `num_slots:i16`, `num_records:i16` |
| 4/6 | anchor type `i16`, alignment `u16` |
| 8/12/16 | total free, contiguous free, free-area offset (`i32`) |
| 20/24 | reserved, flags (`i32`) |
| 28 | GCC bit-field word: `is_saving` bit 0, 31 reserved bits |

For slot `s`, read the little-endian `u32` at user offset `16344 - 4*(s+1)`: bits 0..13 are record offset, bits 14..27 record length, bits 28..31 record type. `offset=0` represents an empty/deleted slot. The compiler-dependence is explicitly acknowledged in source; these bit positions are only guaranteed for the pinned GCC ABI. Slot addressing and bounds are shown at [`src/storage/slotted_page.c:4522-4599`](/home/vimkim/gh/cb/feat-oos/src/storage/slotted_page.c).

Record types 0..7 are unknown, assign-address, home, new-home, relocation, bigone, marked-deleted, deleted-will-reuse; 8..15 are reserved ([`src/storage/storage_common.h:1157-1199`](/home/vimkim/gh/cb/feat-oos/src/storage/storage_common.h)). Validate nonnegative header counts/free space, records <= slots, `cont_free <= total_free`, legal anchor/alignment, slot array within page, nonempty record offsets >=32, `offset+length` before the slot array, no arithmetic overflow, and sensible nonoverlap. Initialization and engine checks are at [`src/storage/slotted_page.c:1048-1090`](/home/vimkim/gh/cb/feat-oos/src/storage/slotted_page.c) and [`src/storage/slotted_page.c:4522-4567`](/home/vimkim/gh/cb/feat-oos/src/storage/slotted_page.c).

## OOS persistent structures

### OOS file and header page

An OOS file is `FILE_OOS=13`, numerable, with page type `PAGE_OOS=8`. All its pages are slotted `ANCHORED` pages aligned to `MAX_ALIGNMENT` (8) ([`src/storage/oos_file.cpp:2079-2094`](/home/vimkim/gh/cb/feat-oos/src/storage/oos_file.cpp)). The file header's `vpid_sticky_first` identifies the OOS header page. Slot 0 of that page is one `REC_HOME` raw `OOS_HDR_STATS`; data pages do not have this record ([`src/storage/oos_file.cpp:991-1058`](/home/vimkim/gh/cb/feat-oos/src/storage/oos_file.cpp)). Do not classify an arbitrary OOS page as a header merely because it has slot 0.

GDB gives `OOS_HDR_STATS` size 264:

| Record offset | Field |
|---:|---|
| 0 | self OOS `VFID` (8) |
| 8/12 | estimated page/record counts `i32` |
| 16 | estimated record-length sum `f32` |
| 20..44 | seven best-space counters/indices `i32` |
| 48 | full-search `VPID` (8) |
| 56 | ten second-best VPIDs (80) |
| 136 | ten `{VPID(8), freespace:i32}` best entries (120) |
| 256/260 | reserved `i32` |

These best-space statistics are non-WAL-logged hints and may be stale ([`src/storage/oos_file.cpp:930-957`](/home/vimkim/gh/cb/feat-oos/src/storage/oos_file.cpp)); use them for display only, never as the allocation truth.

### OOS chunk records and chains

Every live OOS data slot is `REC_HOME` and begins with a **raw native** 16-byte header ([`src/storage/oos_file.hpp:26-38`](/home/vimkim/gh/cb/feat-oos/src/storage/oos_file.hpp)):

```text
offset 0  i32 total_data_length
offset 4  i32 chunk_index
offset 8  OID next_chunk = {pageid:i32, slotid:i16, volid:i16}
offset 16 payload bytes
```

Every chunk repeats the total logical payload length. A single chunk has index 0 and NULL next OID. Multi-chunk insertion writes tail-to-head so links are known, assigns indices `0..N-1`, and returns the index-0 OID ([`src/storage/oos_file.cpp:1402-1494`](/home/vimkim/gh/cb/feat-oos/src/storage/oos_file.cpp)). With this profile, maximum payload per chunk is:

```text
align_down(16344 - SPAGE_HEADER(32) - SPAGE_SLOT(4), 8) - OOS_HEADER(16)
= 16,288 bytes
```

This follows [`src/storage/slotted_page.c:835-844`](/home/vimkim/gh/cb/feat-oos/src/storage/slotted_page.c) and [`src/storage/oos_file.cpp:2315-2324`](/home/vimkim/gh/cb/feat-oos/src/storage/oos_file.cpp).

Validate each slot length >=16; positive total length; head index 0; subsequent indices increase exactly by one; repeated total lengths match; each payload is nonempty; each next OID has an existing volume/page/slot, physical `PAGE_OOS`, and `REC_HOME`; accumulated payload never exceeds and finally equals total length; terminal next is NULL. The engine performs most length/index checks ([`src/storage/oos_file.cpp:1566-1741`](/home/vimkim/gh/cb/feat-oos/src/storage/oos_file.cpp)); an external parser must additionally keep a visited OID set and step/length bounds to reject cycles deterministically.

### Heap-side OOS reference

A heap class header is slot 0 of its heap header page; its raw `HEAP_HDR_STATS` includes the class OID and the OOS `VFID` at ABI offset 32 ([`src/storage/heap_file.c:194-231`](/home/vimkim/gh/cb/feat-oos/src/storage/heap_file.c), [`src/storage/heap_file.c:12414-12497`](/home/vimkim/gh/cb/feat-oos/src/storage/heap_file.c)). This cross-checks the OOS file descriptor's owning HFID/class OID.

Inside an ordinary heap object record, the first representation word has record flag `HAS_OOS=0x08` in the five record-flag bits shifted by 24, but that flag does not alter the 8..32-byte MVCC header size ([`src/base/object_representation_constants.h:140-176`](/home/vimkim/gh/cb/feat-oos/src/base/object_representation_constants.h)). Each variable-offset-table entry reserves low bit `0x1` for OOS and low bit `0x2` for the last-entry sentinel; the actual offset masks both low bits ([`src/base/object_representation.h:439-459`](/home/vimkim/gh/cb/feat-oos/src/base/object_representation.h)). Offset entry width is 1, 2, or 4 bytes as selected by representation-word bits 29..30.

An OOS-marked variable region is exactly 16 big-endian OR bytes:

```text
OID: pageid i32 BE, slotid i16 BE, volid i16 BE
full logical length: signed bigint i64 BE
```

The write path uses `or_put_oid` and `or_put_bigint` ([`src/storage/heap_file.c:12983-13055`](/home/vimkim/gh/cb/feat-oos/src/storage/heap_file.c)); the reader requires 16 available bytes, non-NULL OID, and `0 < length <= DB_MAX_STRING_LENGTH` ([`src/storage/heap_oos.cpp:420-459`](/home/vimkim/gh/cb/feat-oos/src/storage/heap_oos.cpp)). Also require a bounded last-entry sentinel, ordered masked offsets, and each region inside the record, matching [`src/storage/heap_oos.cpp:76-132`](/home/vimkim/gh/cb/feat-oos/src/storage/heap_oos.cpp) and [`src/storage/heap_oos.cpp:185-238`](/home/vimkim/gh/cb/feat-oos/src/storage/heap_oos.cpp). Cross-check inline length with every OOS chunk header and final accumulated payload.

## Corruption policy for a strict read-only inspector

Parsing should be fail-closed per structure but continue elsewhere with explicit diagnostics. At minimum:

- Never trust file paths, counts, offsets, enum values, multiplication, or linked-list pointers before bounds checks; canonicalize/contain volume paths according to the chosen CLI policy.
- Require a stable read snapshot for meaningful results. Offline read-only cannot guarantee consistency if a server is writing; detect changed file size/mtime or page LSA across passes and label the scan inconsistent.
- Check physical prefix identity and duplicated LSA before interpreting plaintext; never parse a TDE ciphertext user area.
- Validate volume geometry, bitmap bounds, file-header accounting, extdata item widths/counts, all referenced VPIDs/OIDs and expected physical types.
- Track visited VPIDs/OIDs for volume chains, extdata chains, heap traversal, and OOS chains. Source assertions often detect only immediate self-links and are not an adequate hostile-input boundary.
- Treat estimates (`HEAP_HDR_STATS`, `OOS_HDR_STATS`) as hints. Derive exact display occupancy from validated slots and exact ownership from file allocation tables.
- Keep three distinct states: sector reserved by volume bitmap, page allocated to a file by its page bitmap/full-sector table, and physical page type/content. Disagreement is diagnostically valuable and must not be silently normalized.

## Corroboration

The locally generated fixture `/home/vimkim/.cub/db/feat-oos/commondb/demodb` matches the contract without being used to derive it:

- `databases.txt` has the five documented whitespace fields; `demodb_vinf` contains IDs `-5..-2,0,1` in ascending order.
- The primary volume is 67,108,864 bytes = 4096 physical pages. Raw page 0 has prefix `ptype=3`, user offset 0 magic `CUBRID/Volume`, user offset 26 value `0x4000`, and identical 8-byte leading/trailing LSAs.
- Running recovered Build ID `50f2e7a451bae7f0c5a889dd51d6ef1d82da0131` read-only with `--plain --no-overlay --rows=2` discovers 2 volumes, 97 files, sector reservations, per-file ownership, and physical categories. Its recovered README describes the same stages ([`recovered/README.md`](/home/vimkim/temp/volmap/recovered/README.md)).

The recovered executable is statically linked and its behavior corroborates discovery, header checks, sector maps, file-header/extdata walking, and deep scans. It does **not** supersede the pinned source for offsets, OOS semantics, checksums, encryption behavior, or corruption rules.

## Contract boundary

This report is sufficient to pin parser fixtures and decisions for this one ABI/commit. It deliberately does not claim compatibility with a different CUBRID commit, 32-bit build, Windows compiler, big-endian machine, alternate page size, or future OOS header revision. Such inputs should be rejected as unsupported until separately versioned from authoritative source and fixtures.
