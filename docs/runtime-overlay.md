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
