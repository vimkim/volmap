Type: grilling
Status: resolved
Blocked by: 05, 15

# Decide whether AOUT history belongs in this overlay

## Question

Does “recently evicted” AOUT observation belong in this state-only overlay
handoff or a later effort? If included, decide its independent capture,
retention, coverage and resource semantics without presenting it as current
residency or silently expanding the accepted producer/broker budgets.

## Comments

Graduated from the map's AOUT fog after the resource-budget resolution.
At creation, inclusion or exclusion was undecided. The user-approved scope
outcome is recorded below.

Read-only evidence from CUBRID source commit
`cd593bcf2d8643b4698f1cb311c4c23af23a9d57`, worktree
`/home/vimkim/gh/cb/pgbuf-bcb-report` (inspected source files unmodified):

- `src/base/system_parameter.c:10112` forcibly sets the AOUT ratio to zero;
  `src/storage/page_buffer.c:5724` returns with zero capacity. Current normal
  initialization does not provide populated AOUT observations.
- `src/storage/page_buffer.c:620` stores VPID, former LRU index and links,
  not timestamps or an eviction event sequence. Initialization bounds the
  FIFO to at most 32,768 entries (`:5716`); insertion recycles its oldest
  nodes (`:10357`).
- Insertion occurs before selected BCB reuse finishes (`:9358`, `:15523`);
  removal occurs during later void-zone unfix/unlatch (`:6784`, `:6804`).
  Membership therefore does not establish current buffer-pool absence.
- The AOUT mutex guards insertion, lookup/removal and recycled node lifetime
  (`:629`, `:10375`, `:10455`); it does not establish an atomic joint
  AOUT-plus-BCB view. This is not simply another lock-free BCB field.
- Local knowledge-base context was checked in
  `/home/vimkim/gh/my-cubrid-docs/pgbuf-analysis/research/cubrid-lru-victim.md:323`
  and `:357`; its older-source findings were rechecked against the commit above.

## Answer

Closed as out of scope with the user's “all recommended” acceptance.

Defer AOUT collection, wire fields and history visualization to a separate
future effort. Leave engine disablement unchanged and do not infer eviction
events from partial-scan differences. Keep the accepted resident-state
overlay, wire contract, resource budgets and verification gates unchanged.
Future work must first verify safe engine support, then specify bounded AOUT
membership observations and honest coverage/age semantics. AOUT membership is
neither timestamped eviction history nor proof of current nonresidency.

No AOUT collector, capability, heatmap layer, history cache, test requirement
or engine re-enablement is added to this handoff. This is a scope exclusion,
not a permanent rejection of future AOUT work. It belongs in the map's Out of
scope section, not Decisions so far. Flush-transition scope remains a
separate open decision; this acceptance does not resolve it.
