# Selected-page buffer observations

Start a loopback viewer with `--runtime-page-buffer --runtime-socket PATH`,
select a Page, enable observations, and choose **Refresh selected-page
observation**. The producer socket must be in an owner-only directory (0700)
and have mode 0600. Both peers must have exactly the same effective UID.
Remote operators can forward the loopback viewer through SSH.

The broker verifies database creation and the complete permanent-volume
identity set before requesting a scan. A refusal or incompatible protocol
clears evidence; another explicit refresh retries attachment. Observations
remain separate from observed disk state, inspection revisions and exports.

A resident mark means observed residency during the displayed capture interval.
A complete-scan omission means observed nonresidency; partial omissions,
duplicate VPIDs and unevaluated pages remain unknown. Neither state proves
current residency, page-image correspondence, transaction visibility,
durability or event causality. Latch and LRU tuples are independently coherent;
records and scans are not atomic. List indices belong to one incarnation.

The selected-page freshness interval is 500 ms. Age includes the broker's
scan-request start through serialization, the browser's full HTTP round trip,
and elapsed browser time. Broker serialization has a 100 ms checked window;
the transmitted age includes that window and integer rounding. Cache reads do
not renew age. Evidence expires by 30 seconds, including while paused. A
browser clock discontinuity can expire evidence earlier. Pause, navigation,
generation changes and disabling observations revoke pending adoption.

## Bounds and ownership

One session owns one connection, one latest capture, and at most one unfinished
scan. Scan starts are separated by at least 500 ms. Eight requests, including
waiters and response bytes still held by HTTP, have admission slots; there is
no additional observation queue. HTTP bodies are at most 64 KiB, scopes at most
512 ordered VPIDs, and serialized responses at most 1 MiB. Observation work has
a 2.5-second deadline, attachment 500 ms, and a scan exchange two seconds.

The decoder processes individual bounded JSON lines. Frames include their LF:
4 KiB controls/records, 64 KiB handshake, depth 16, 65,536 raw records and
64 MiB framed scans. Unknown same-major fields and in-scan frame kinds count
against these bounds. Duplicate JSON members are invalid. A missing footer,
malformed frame or resource refusal discards the unfinished assembly rather
than publishing its prefix.

Memory uses conservative reservations, charged before allocating the associated
objects. A shared atomic budget refuses reservations exceeding 128 MiB:

| Owner | Reservation | Included allocations |
| --- | ---: | --- |
| Session | 8 MiB | bounded expected identities, paths, request parsing, scope copies, metadata and expiry tasks |
| Connection decoder | 16 MiB | 64 KiB frame capacity, bounded JSON DOM and transient normalization |
| Each capture | 32 MiB | fixed record slab, capture/handshake metadata and sorting workspace |
| Each admitted response | 2 MiB | selected-row projection and 1 MiB serialization capacity |

Records contain scalars, static vocabulary and inline nullable LSAs; they have
no per-record heap allocations. A compile-time bound limits each record to
256 bytes, so the 65,536-entry slab is at most 16 MiB. The 32 MiB capture
reservation also covers metadata and stays below the 48 MiB decoded-scan limit.
The per-frame JSON reservation conservatively covers container capacities and
transient strings for the pinned parser and toolchain, including ignored fields.

Publication temporarily owns the old and new captures. Serialization borrows
one capture while holding the session lock and produces independently owned
bytes; HTTP does not hold an old capture reference. Those bytes retain their
reservation and admission permit until released. The normal maximum concurrent
reservations are 8 + 16 + 2×32 + 8×2 = **104 MiB**, below the aggregate ceiling.
No replacement uncharges a surviving object. There is no spill or observation
history. Reservations are allocation accounting, not an RSS measurement.

## Reproducible verification

The producer-owned corpus is copied verbatim under
`fixtures/pgbuf-inspector/v1`; its provenance and offline checksum instructions
are in `fixtures/pgbuf-inspector/PROVENANCE.md`. Rust tests exercise the real
incremental decoder, private sockets, HTTP boundary and highest broker interface.
Credential isolation requires Linux user namespaces and subordinate UID/GID
mappings; it fails instead of skipping when that environment is unavailable.

Browser tests use the actual Volmap server and a scripted Unix-socket producer
whose semantic records come from the pinned corpus. They do not replace the
HTTP response with browser fixtures. Real CUBRID integration and release
performance evidence remain tickets 07 and 08; this slice does not claim them.
