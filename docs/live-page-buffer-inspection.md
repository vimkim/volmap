# Live page-buffer inspection (future improvement)

> Status: design reference only. This is not part of Volmap's current offline,
> immutable-snapshot contract.

Volmap could optionally cooperate with a debug-enabled `cub_server` to inspect
resident page-buffer state and compare it with persistent page images. The
recommended design is a server-owned semantic inspector over a local Unix-domain
socket. The wire interface should expose stable evidence and classifications,
not CUBRID structures, pointers, flag words, or raw application page contents.

## Recommended seam

```text
Volmap
   |  VPID, semantic state, digests, classification
   v
transport adapter (gRPC/UDS or framed Protobuf/UDS)
   v
ResidentPageConsistencyInspector in cub_server
   +-- non-loading page-buffer capture
   +-- direct main-volume observation
   +-- DWB observation
   +-- TDE normalization
   `-- consistency classification
```

Start with one bounded operation:

```text
InspectPage(VPID) -> PageObservation
```

`PageObservation` should include a server-incarnation-bound capture token,
residency, semantic buffer state, page and oldest-unflushed LSAs, normalized
memory/main-volume/DWB digests, evidence limitations, and one of these explicit
classifications:

- `SYNCHRONIZED_WITH_MAIN_VOLUME`
- `SYNCHRONIZED_WITH_DWB`
- `DIRTY_AHEAD_OF_PERSISTENCE`
- `FLUSH_TRANSITION`
- `UNEXPECTED_CLEAN_MISMATCH`
- `NOT_RESIDENT`
- `INDETERMINATE`

A single `synchronized` boolean is insufficient. Main-volume visibility, DWB
staging, filesystem synchronization, and physical durability are different
claims. Durability must remain `UNKNOWN` unless the debug feature records the
corresponding DWB and volume synchronization epochs.

## Per-page capture contract

The inspector must not load a missing page or silently use a buffered read for
the persistent image. For a resident page, use an optimistic bracket:

```text
capture protected resident image A
        |
        v
read main volume directly; inspect DWB; normalize TDE
        |
        v
capture protected resident image B
        |
        v
A and B stable? classify : return transitional/indeterminate
```

Page protection should cover only semantic BCB capture and one page copy. Do
not hold a BCB mutex or page latch across volume I/O, hashing, allocation,
transport writes, or backpressure. Latch acquisition must be conditional and
deadline-bounded. Prefer a dedicated diagnostic resident lookup over the normal
fix/unfix path so observation does not alter cache-hit accounting or LRU state.

Coherence is per page. A resident-pool traversal is not a database-wide atomic
snapshot and must never be presented as one.

## Transport and safety

Keep transport outside the storage-engine implementation. gRPC over a
Unix-domain socket is reasonable if CUBRID already accepts the gRPC++ build
cost; otherwise use a small length-prefixed Protobuf adapter over the same kind
of socket. An in-memory adapter should exercise the identical semantic
interface in tests.

Recommended gates and defaults:

- Dedicated compile-time debug feature plus explicit runtime enablement.
- Per-database socket in a server-owned `0700` directory, normally mode `0600`.
- Authenticate the peer with `SO_PEERCRED` and bind the handshake to the
  database and server incarnation.
- No TCP listener by default; remote use can go through SSH forwarding.
- Digest-only evidence in the first version. Never return TDE keys, pointers,
  C structure layouts, or raw page payloads.
- Hard page, time, byte, latch-wait, concurrency, and queue limits with
  cancellation and backpressure.
- Semantic, versioned wire fields; never serialize `PGBUF_BCB` or
  `FILEIO_PAGE` directly.

## Candidate extensions

1. Add bounded multi-VPID and resident-set streaming. State-only scans should
   avoid page copies and disk reads; persistent comparison should normally be
   targeted.
2. Let Volmap optionally read the main volume independently and compare its raw
   digest with the server's disk observation. This improves fault independence,
   but it is a cross-check rather than the primary classification because the
   volume can change between observations.
3. Add a bounded, non-blocking transition changefeed for `RESIDENT_LOADED`,
   `CLEAN_TO_DIRTY`, `FLUSH_PREPARED`, DWB transitions, main-volume write/sync,
   and eviction. Use it to explain causality after obtaining a snapshot
   baseline; event-only reconstruction is invalid after attach-late or a gap.
4. Keep `SHOW PAGE BUFFER STATUS` for approximate aggregate monitoring. Its
   lock-free BCB scan is not the consistency primitive for page-level evidence.

Shared-memory export, raw process-memory traversal, and global stop-the-world
snapshots are not preferred starting points. They expose private layout,
increase security risk, or impose substantially more synchronization complexity
than the semantic inspector.

## Suggested delivery order

1. Digest-only `InspectPage(VPID)` with `NOT_RESIDENT` and bounded capture.
2. Main-volume, DWB, and TDE-aware evidence with conservative classifications.
3. Stability bracketing or debug-only mutation/residency generations.
4. Bounded multi-page streaming and state-only resident scans.
5. Optional independent Volmap disk cross-check.
6. Optional transition changefeed for flush-path diagnosis.

