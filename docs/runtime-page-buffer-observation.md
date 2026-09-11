# Selected-page buffer observations

Start a viewer on loopback or an explicit IPv4 address with `--runtime-page-buffer --runtime-socket PATH`,
select a Page, and enable observations. Sampling runs every 500 ms after each
completed request; **Refresh selected-page observation** also provides an
explicit attachment retry. The producer socket must be in an owner-only directory (0700)
and have mode 0600. Both peers must have exactly the same effective UID.
Wildcard runtime listeners are rejected. Direct LAN serving uses unauthenticated
plain HTTP; remote operators can also forward a loopback viewer through SSH.

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

## Polling, pause, and recovery

Transient failures retain previously observed evidence with its original age.
Retries use nominal delays of 0.5, 1, 2, 4, then 8 seconds, with ±20% jitter
and a 9.6-second maximum timer delay. A valid scan resets retry backoff. Source
connectivity and observation freshness remain separate. Peer/identity refusal,
producer incarnation changes, and incompatible protocols clear evidence and
stop automatic retry until the operator explicitly refreshes.

Hidden tabs send no runtime requests. Pause freezes adoption of runtime captures
and newer disk generations. Visible paused tabs check capability metadata every
five seconds, without requesting scans. A newer capture from another tab is an
availability offer only. Metadata rechecks socket ownership, detects a closed
or replaced producer connection, and verifies any replacement handshake before
reporting its incarnation. Disk identity changes invalidate retained evidence;
a generation number alone does not invalidate same-identity state observations.

Resume requests a scan started after the resumed request reaches the broker,
which conservatively satisfies the browser's resume boundary. Cached or already
in-flight pre-resume scans cannot fulfill it. The shared 500 ms scan-start floor
still applies, independently of the producer's 100 ms floor. Cancelling one
caller preserves other admitted demand; the last departing caller cancels an
unfinished refresh. Epoch and scope checks reject late results independently
of network cancellation.

Observation POST bodies accept `cadence_ms` (default 500, bounded to 100–30,000)
and `after_request` (default false). Cadence governs cache eligibility, not the
shared scan-start floor. Capability metadata supplies a monotonic session
`revision`, sanitized `incarnation_binding`, and optional `capture_identity`;
it never supplies page evidence for paused adoption. Busy, rate-limited, and
defensive parameter-off responses have stable normalized reasons. Socket
absence remains unavailable and never implies parameter-off.

Both broker and browser compare elapsed wall time with monotonic elapsed time
only to detect clock discontinuity or suspension. Discontinuities can expire
evidence early; wall time cannot renew it or bypass the scan-start floor.
Reusing a capture cannot lower its previously established conservative age.

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
whose semantic records come from the pinned corpus. Lifecycle fault tests also
intercept browser requests to exercise retry and HTTP protocol incompatibility;
separate real socket and HTTP cases prove authentication, framing, ownership,
restart and resume behavior. The test producer accepts a test-only SIGHUP restart,
and process-wide fault scenarios run serially across Chromium and Firefox. Real CUBRID integration and release
performance evidence remain tickets 07 and 08; this slice does not claim them.
