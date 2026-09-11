# Browser budget revision verification — 2026-09-11

The [explicit policy revision](../../browser-memory-budget-revision.md) changes
browser incremental peak RSS from 32 MiB to **64 MiB per tab**, targeting ordinary
developer PCs with 1–8 tabs. The numeric selection is an engineering allowance;
the user confirmed the usage profile, not a measured capacity guarantee.

## Original density harness, new recorded ceiling

Executed once: `VOLMAP_DENSITY_PORT=41744 just vite::frontend-density`.
Both cases passed (exit 0):

| Browser | State increment | LRU increment | Worst input-to-visible p95 |
| --- | ---: | ---: | ---: |
| Chromium | 15.35 MiB | 29.87 MiB | 63.4 ms |
| Firefox | 25.62 MiB | 28.18 MiB | 62 ms |

Each report records `browserMemoryBudgetBytes: 67108864`. The measurement remains
12,288 actual laid-out cells, 1920×1080, 40 Tab interactions per arm, two
order-alternated state/LRU pairs and 50 ms sampled summed browser-process-tree
RSS, with a fresh one-tab browser per arm. Input p95 must remain ≤100 ms.
Only the ceiling/report field changed; no runtime optimization was made.

The consumer checkout was `75e6bb75155ed6ea61f7340ccdb2285768866bc2` plus the
retained [test patch](density-test.patch.gz). The [manifest](manifest.json) pins
87 source/test inputs; only the density test differs from the preceding memory
growth evidence. Browser versions, host, corpus hash, build and per-arm raw
samples are in [the raw archive](density-raw.tar.gz) and browser JSON reports.
The whole-checkout dirty hash also includes policy documentation and an unrelated
producer ticket; it is not the test-patch hash. [Command log](density.log.gz).

## Interpretation

This is a local synthetic single-tab result, not the real-engine 1/8/32-tab
release matrix or repeated-use characterization. The ordinary observations
happen to be below the old ceiling too; that does not cancel earlier 32 MiB
failures. Old manifests, archives and verdicts remain unchanged.

The [20-cycle diagnostic](../08-memory-growth/README.md) still leaves retained
memory growth unresolved. Its roughly 66.66 MiB post-GC control/toggle difference
uses a different method and cannot be treated as a peak-RSS gate result.
Manual accessibility, dedicated-host workload qualification and repeated-use
investigation remain open. Release readiness is **false**.
