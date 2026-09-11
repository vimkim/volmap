# 02: Attach securely to the correct server

**What to build:** An operator explicitly enables the inspector and an authorized local client verifies the correct database and server incarnation through a private, bounded handshake. Disabled or failed activation leaves normal database service intact.

**Blocked by:** 01 — Make wire v1 executable.

**Status:** done

- [x] Add startup-only enable_pgbuf_inspector=false with PRM_FOR_SERVER and PRM_HIDDEN only, read once at initialization. Disabled creates neither daemon nor socket, no scans and no page-buffer hot-path parameter reads. No reload, user-change or client-synchronization flags are added.
- [x] Compile state-only support into supported Unix SERVER_MODE Release, RelWithDebInfo, Debug and OptDebug builds without a new inspector build option or macro. Windows/non-server binaries expose no endpoint; unsupported peer-authentication environments cannot fall back to weaker security.
- [x] Own AF_UNIX SOCK_STREAM JSON-lines through a dedicated inspector daemon using existing lifecycle conventions. Initialize after required buffer/identity/thread services and cancel clients before those services are torn down. Keep normal startup and shutdown working in debug and release.
- [x] Use the existing CUBRID Unix-socket root and dedicated pgbuf-inspector directory at 0700 with socket mode 0600. Use a bounded opaque key derived from canonical database path and creation identity; report the explicit socket location only through local operator diagnostics.
- [x] Authenticate clients with Linux SO_PEERCRED before protocol exchange, requiring exact effective-UID equality without root/group exceptions. Preserve symlinks, wrong owners, non-sockets and active sockets; reclaim only confirmed stale same-owner sockets, never indeterminate failures. Unlink at shutdown only if the pathname still identifies the created socket.
- [x] Path length, ownership, permissions, reclamation, bind or listen failure produces a prominent stable local startup error, leaves the inspector unavailable for the incarnation, and allows database startup. No background bind retries occur.
- [x] Handshake supplies negotiated version, unpredictable incarnation, shared/private LRU topology, database creation and the complete persistent-volume set of volid, volume creation, device and inode. Exclude producer-only temporary volumes from identity proof. Refuse an oversized identity set instead of truncating it; names/paths alone are insufficient.
- [x] Apply two-client admission, 64 KiB handshake/output buffering, 4 KiB control frames, nesting depth 16, 500 ms attachment deadline and 250 ms write-stall disconnection from the first endpoint. Validate versions and malformed input before allocation. No unbounded client queue or provisional unsafe endpoint is acceptable.
- [x] Exercise actual sockets for default-off absence, successful identity-bound handshake, unsupported version, peer refusal, private permissions, activation failure and exact-socket cleanup. Include copied-database identity evidence and restart incarnation change. No paths, UID/GID/PID or raw OS errors appear in observation frames.
- [x] Scan collection is ticket 03. This attachment slice must not return a fabricated empty complete scan when collection is absent. Preserve CUBRID error handling, formatting and include conventions, and record real executable checks with exact build revisions.

## Source and scope

Implements the approved CUBRID producer specification for CBRD-27398 in this feature tracker. Read its full contract and linked decision authority before implementation. The public protocol is the main test boundary; focused deterministic scan/serializer checks supplement real I/O. This ticket does not add resident-page capture, AOUT or event history, native LRU telemetry, or Volmap UI work.

## Comments

2026-09-09: Published after the user approved the eight-ticket breakdown and blocking edges. Ready-for-agent describes triage readiness; blockers and acceptance criteria remain unsatisfied until evidenced.

2026-09-09: Implemented and committed as `9e942a812f1451cf81308886c5bb8e17a0b44b98` on
`CBRD-27398-pgbuf-inspector-contract` (tested tree `55b0c00e2238ea883f742604de06659fa976fe36`).
Debug and release/RelWithDebInfo each passed 27/27 configured CTest targets;
each inspector run executed 36 cases / 50,747 assertions, plus three mapped-UID
credential cases / 32 assertions. Five real-server scenarios passed in each mode,
including every persistent volume's file identity, connected-client shutdown,
restart incarnation, copydb and conflicting-path startup with working SQL.
Standards and Spec reviews have zero remaining findings; pre-commit formatting passed.
The release suite used a short private CUBRID_TMP because the default PL fixture
socket path exceeded the Unix limit. Exact revisions, commands, build modes,
resolved failures, binary hashes and raw logs are in
[verification/02/manifest.json](../verification/02/manifest.json).
The worktree is restored to debug_gcc; pre-existing CCI changes remain untouched.
Ticket 03 is now unblocked. No source push, PR or JIRA publication was performed.
