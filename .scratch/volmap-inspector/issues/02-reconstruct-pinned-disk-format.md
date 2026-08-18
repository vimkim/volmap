Type: research
Status: resolved
Blocked by:

# Reconstruct the pinned feat/oos disk-format contract

## Question

What exact on-disk contract must a read-only external inspector implement for `/home/vimkim/gh/cb/feat-oos` commit `e1e651debf6cc100172bde96603b17424f9c135a`? Trace primary source and generated fixtures to document database and `_vinf` discovery, volume headers, I/O versus database page boundaries, sector allocation tables, file headers and extensible tables, file ownership and file types, physical page types, generic slotted-page headers and slot entries, and OOS file/page/chunk/value-chain structures. Separate persistent bytes from in-memory-only C/C++ layout; identify checksums, encryption/TDE limitations, endianness and alignment assumptions, corrupt-input invariants, and any facts the recovered `volmap-standalone` can validate. Produce a source-and-commit-cited format inventory suitable for later parser and test decisions.

## Comments

## Answer

Resolved in [the pinned disk-format contract report](../research/pinned-disk-format.md). It pins the x86-64/GCC byte layouts and invariants for discovery, physical pages, volume/sector metadata, file ownership and extensible tables, slotted pages, heap-side OOS references, OOS files/chunks/chains, TDE and checksum limits, and explicitly separates raw native-layout bytes from big-endian object-representation bytes. Pinned source is authoritative; generated fixture and recovered-binary results are identified only as corroboration.
