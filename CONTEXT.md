# Volmap Inspector

Volmap Inspector is a read-only offline explorer of CUBRID volume allocation and page structure. This glossary separates physical storage facts from interpretations presented by its CLI, TUI, and web viewer.

## Storage hierarchy

**Volume**:
A CUBRID volume file belonging to the inspected database snapshot.
_Avoid_: Disk, database file

**Sector**:
A fixed physical region of 64 consecutive pages in a volume.
_Avoid_: Block, extent

**Sector summary**:
The derived reservation, allocation, ownership, utilization, and anomaly counts for one sector.
_Avoid_: Sector status

**Page classification**:
The tool's evidence-backed description of a page as volume metadata, unreserved, reserved but unallocated, allocated to a file, unreadable, or inconsistent.
_Avoid_: Page status

**Page ownership**:
The file identity and logical file type associated with an allocated page by CUBRID file-allocation metadata. Ownership is distinct from the page's physical page type.
_Avoid_: Page kind

## Page inspection

**Slotted page**:
A CUBRID page whose records are addressed through a slot directory and occupy byte extents within the page body.
_Avoid_: Record page

**Slot entry**:
A slot-directory entry describing a record's slot identifier, byte offset, length, and record type, or describing an empty/deleted slot.
_Avoid_: Record pointer

**Page byte map**:
A physical visualization of the page header, occupied record extents, alignment waste or gaps, contiguous free area, and slot directory across the page's byte range.
_Avoid_: Page status map

**Deep inspection**:
Opt-in decoding of a selected page's header, slot directory, record allocation, and recognized page-type-specific metadata without exposing user values.
_Avoid_: Deep scan

## OOS storage

**OOS page**:
A slotted page with physical page type `PAGE_OOS` in an OOS file.
_Avoid_: Overflow page

**OOS chunk record**:
One physical slotted-page record containing an OOS record header and a payload fragment.
_Avoid_: OOS page, OOS record

**OOS value chain**:
One or more linked OOS chunk records containing one complete serialized attribute value.
_Avoid_: OOS chain page

## Distribution

**Standalone executable**:
The single Linux x86-64 `volmap` binary, with no runtime dependency on glibc, CUBRID libraries, installation assets, network services, or separately installed web assets.
_Avoid_: Portable installation

## Evidence governance

**Format authority**:
The pinned CUBRID source revision and company-generated fixtures from which supported persistent layouts and invariants are established.
_Avoid_: Legacy implementation

**Recovered artifact**:
A legacy executable or reverse-engineering output obtained from another CUBRID employee and kept outside Volmap Inspector source and distribution.
_Avoid_: Reference implementation, source code

**Behavioral oracle**:
An optional, explicitly authorized black-box comparison that records normalized observable facts from a recovered artifact; it is never a format authority.
_Avoid_: Golden implementation, compatibility source
