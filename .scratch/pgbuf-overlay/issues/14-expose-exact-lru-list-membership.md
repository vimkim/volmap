Type: grilling
Status: resolved
Blocked by: 05

# Expose exact LRU-list membership in wire v1

## Question

[Define wire contract v1](05-define-wire-contract-v1.md) included the semantic
LRU zone but excluded the list index as engine-internal quota detail. Reconsider
that exclusion: should every resident BCB observation also identify the exact
shared or private LRU list that contained it when its flags were sampled?

CUBRID packs the LRU zone and global LRU-list index into the same `flags` word
and changes them together with a compare-and-swap. A producer can therefore
load `flags` once and map the pair to semantic fields without traversing or
locking an LRU list:

- `lru_list_kind`: `shared | private | none`
- `lru_list_index`: kind-local non-negative index, absent for `none`
- the existing `lru_zone`: `lru1 | lru2 | lru3 | void | invalid`

The contract must state that `(lru_list_kind, lru_list_index, lru_zone)` is
coherent within that one flags load but remains a volatile observation that may
change immediately. `void` and `invalid` BCBs have no LRU-list membership.
Never expose the packed global index or raw flag bits.

Also decide whether v1 needs only per-BCB membership or a separate per-list
summary. A summary derived by grouping emitted BCB observations needs no LRU
mutex and is internally attributable to the non-atomic scan; live list
counters, quotas, and thresholds have different synchronization semantics and
must not be presented as the same snapshot without a separate capture design.

## Comments

Raised by the user on 2026-09-04 after resolving the gating matrix: the desired
information is not merely LRU1/LRU2/LRU3 zone, but exact private/shared LRU-list
membership for each BCB.

## Answer

Resolved with the user, 2026-09-05. Wire v1 exposes semantic, per-BCB
membership without acquiring an LRU mutex or traversing an LRU list:

- Every resident BCB record adds `lru_list_kind`
  (`shared|private|none|invalid`) and `lru_list_index`. The index is a
  non-negative, kind-local index for `shared` and `private`; it is `null` for
  `none` and `invalid`. `lru_zone` remains a separate field.
- The handshake adds incarnation-scoped `shared_lru_count` and
  `private_lru_count`. List indices describe this topology and are not stable
  across server incarnations or restarts, including restarts with a different
  topology configuration.
- The producer loads a BCB's packed `flags` word once and decodes
  `(lru_list_kind, lru_list_index, lru_zone)` from that one sample. The tuple
  is coherent within the load but remains a volatile observation that may
  change immediately; the enclosing pool scan is still non-atomic.
- For an `lru1|lru2|lru3` BCB, let `global_index` be the packed LRU index. It
  maps to `shared, global_index` when
  `global_index < shared_lru_count`; to
  `private, global_index - shared_lru_count` when
  `global_index < shared_lru_count + private_lru_count`; and to `invalid,
  null` otherwise. A non-LRU-zone BCB maps to `none, null`. The explicit upper
  bound prevents an out-of-range packed value from being mislabeled private.
- Volmap may derive per-list observed counts by grouping the emitted BCB
  records. Those counts describe this scan only; if the scan footer says it
  was truncated, the counts are partial.

Wire v1 does not expose the packed global index, raw flag bits, native live-list
counters, quotas, thresholds, ticks, or list pointers. Those values have
different synchronization and stability properties and require a separate
capture design if later needed.

This decoding surface was re-verified on both CUBRID targets: current develop
(`cd593bcf2d8643b4698f1cb311c4c23af23a9d57`) and the Volmap OOS pin
(`e1e651debf6cc100172bde96603b17424f9c135a`). Both pack zone and index into one
flags word and change them together with a whole-word compare-and-swap, so one
semantic contract serves both branches.
