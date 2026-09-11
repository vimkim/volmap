# Reading page-buffer observations

Enable observations in an explicitly attached loopback live viewer. The optional
CUBRID page-buffer source is independent of observed disk state. It neither
revises inspection facts nor adds data to exports or the TUI.

The default **State marks** mode preserves allocation colors and occupancy
patterns at both Volume and Sector scales. A cyan inset outline (`◉`) means
observed resident, an amber corner (`D`) means dirty, and a static magenta bottom
edge (`F`) means flushing. Finding outlines remain independent. There is no
animation or inferred flush timeline.

**LRU topology** changes the cell color meaning to the observed zone: `1`, `2`,
`3`, `V` (void), `!` (invalid), or `?` (unknown). A dashed inset distinguishes
private membership. Unknown and nonresident cells use a neutral background in
this mode. Switching back restores storage colors. If there is no usable source
or evidence expires, the map shows storage colors without runtime marks.

The source state, conservative capture age, sampling interval and pause status
are separate. A fresh observation is within two caller intervals; it is not a
claim of currentness or atomicity. Pausing stops adoption but still allows age
expiry and server-incarnation invalidation. A transient source failure hides map
marks while independently labelled retained capture detail can remain until it
expires. Polling and age updates do not announce repeatedly or move focus.

`○` means an evaluated page was absent from a complete producer scan. `?` means
unknown: a partial-scan gap or a page not evaluated in this request. `≠` means
ambiguous duplicate VPID observations. None of these means source unavailable.
The visible-page panel reports admission, evaluated/requested counts and producer
completeness separately. Rotating requests never accumulate into a pool snapshot.

Shared/private topology counts come from the verified capture's server
incarnation. The expandable per-list summary counts only resident records in the
current bounded HTTP batch. Even a complete producer scan does not make these
scope-limited counts pool totals. Partial producer scans explicitly yield partial
summaries; no native counter, quota or cross-capture history is implied.

Select a Page for exact kind-local list indices and sampled latch mode, waiter
presence, fix count, semantic page kind, dirty/flushing/async-flush-requested/
to-vacuum flags and log positions. A `none` list kind means no list; `invalid`
means unclassified membership. A null index is not applicable; an absent field is
unknown. Exact indices have meaning only within one server incarnation. These
states do not prove image correspondence, commit visibility, durability, causes,
transition boundaries, durations or event counts.

The mode selector and map navigation work with the keyboard. Sector buttons
summarize their observations; the observation disclosure table and Sector grid
expose individual page classifications. Glyphs, edge shapes, accessible names
and detail text supplement color, including in forced-colors mode.

## Enable an actual producer

Use a producer build that implements the v1 inspector protocol. In the target
server's startup configuration, explicitly set:

```ini
enable_pgbuf_inspector=yes
```

The hidden server Boolean defaults to false and is read only at startup; changing
it requires a server restart through the operator's normal database procedure.
Disabled servers create neither inspector daemon nor socket. A startup activation
failure leaves SQL available but observations unavailable for that incarnation;
there is no background bind retry. An absent socket does not reveal the parameter
value. The verified producer matrix is Linux server Debug and RelWithDebInfo;
Windows and non-server binaries provide no inspector endpoint.

Use the exact socket path printed by the producer's `PGBUF_INSPECTOR_READY:`
startup message. The producer creates its private `pgbuf-inspector` directory
beside the master Unix socket and derives a database-specific socket name; do not
guess it from a database name or reuse a socket belonging to another database.
Run Volmap as the same effective UID, with access to the original declared data
volumes. The endpoint directory must be 0700 and socket 0600. Root/group
membership does not replace exact-UID authentication. Pass the path explicitly:

```sh
volmap serve --vinf /data/demodb_vinf --format-profile develop \
  --listen 127.0.0.1:8080 --runtime-page-buffer \
  --runtime-socket /actual/path/from/producer/startup.sock
```

Choose `develop` for the independently tested develop layout or `feat-oos` for the
format-aligned producer; wire compatibility does not select a persistent format.
The [real-producer verification guide](runtime-overlay-verification.md) records
the exact tested commits and bounded disk corpus. Do not assume arbitrary CUBRID
releases or copied snapshots have compatible layouts or runtime identities.

For remote use, run the viewer on the database host and forward its loopback HTTP
port from your workstation:

```sh
ssh -N -L 8080:127.0.0.1:8080 user@database-host
```

Open `http://127.0.0.1:8080` locally, select a Page and enable observations. This
forwards HTTP, not the producer's Unix socket. Keep the same port to preserve the
expected Host/Origin. An explicit IPv4 LAN listener is also supported under
[ADR 0006](adr/0006-runtime-observations-are-loopback-web-capabilities.md); it is
unauthenticated plain HTTP accessible to anyone who can reach that address/port.
Wildcard runtime listeners remain refused.

Before publishing observations the broker verifies peer credentials, database
creation, all permanent-volume identities and server incarnation. Copied volumes
can remain inspectable as disk input while runtime attachment is refused. On
refusal or incompatibility, check the installation, identities and endpoint; use
explicit Refresh after correction. Transient failures retry with bounded backoff.
Restart invalidates retained evidence, including while paused. See
[age, expiry and resource bounds](runtime-page-buffer-observation.md): normal
cadence is 500 ms, stale begins beyond two intervals, and retained evidence expires
by 30 seconds even while paused. Resume requests a new capture; hidden tabs do
not send observation requests. Cached reads never renew capture age.

Observed disk state and page-buffer observation are independent. Matching VPID,
sequence, recency, log positions or sampled flushing does not prove currentness,
page image correspondence, commits or durability. This overlay requires no
resident-page inspection and supplies no AOUT history, transition events, runtime
TUI/export/inspection-graph facts, new kernel-cache features, automatic discovery,
public producer transport, engine writes or hot-path instrumentation.

The [release evidence guide](runtime-overlay-release.md) lists the remaining
manual and performance gates and links their manifest. Functional integration
completion alone does not establish release readiness.
