# Is the Chromium memory-budget failure a leak?

**A memory leak has not been established.** The demonstrated failure is an
incremental browser RSS budget exceedance: in the original density run,
LRU-enabled peak minus matched disabled peak was 34.54 MiB, exceeding 32 MiB.
Those arms use different fresh browser processes. That measurement does not show
whether memory accumulates across repeated use in one browser.

This follow-up at `ffc4cf3` adds a bounded same-browser experiment to answer that
question. It does not repair or close the original gate. The unchanged original
Chromium density command was also rerun and passed once; the prior failure and
its variability remain authoritative.

## Repeated-use diagnostic

Two fresh Chromium browsers execute sequentially, one tab each, with the existing
release server and 12,288 laid-out page cells at 1920×1080. The first performs
20 disabled-control cycles. The second performs 20 enable/LRU/disable cycles.
Each cycle performs the same 40 Tab inputs, then waits one second after disabling
(or while still disabled in the control). RSS is read from the browser process
tree. CDP supplies JavaScript heap usage and DOM/document/listener counts.

Forced garbage collection at cycles 0/5/10/20 separates some uncollected temporary
objects from retained objects. It changes the diagnostic workload and is **not
allowed to replace the original acceptance measurement**. These RSS readings are
checkpoint snapshots, not 50 ms-sampled peaks. Each case's passing test status means
only that all 20 cycles completed; there is no new leak-free assertion or relaxed
memory acceptance threshold.

All values below are measured after disabling and forced GC, in MiB:

| Arm | Cycle | Browser-tree RSS | JS heap used | DOM nodes | Event listeners |
| --- | ---: | ---: | ---: | ---: | ---: |
| Disabled control | 0 | 606.79 | 10.32 | 13,934 | 370 |
| Disabled control | 5 | 644.89 | 10.42 | 13,938 | 370 |
| Disabled control | 10 | 646.17 | 10.43 | 13,938 | 370 |
| Disabled control | 20 | 648.90 | 10.45 | 13,938 | 370 |
| Toggle LRU | 0 | 620.59 | 10.37 | 13,934 | 375 |
| Toggle LRU | 5 | 707.88 | 11.86 | 13,942 | 370 |
| Toggle LRU | 10 | 712.84 | 12.01 | 13,942 | 370 |
| Toggle LRU | 20 | 715.56 | 12.20 | 13,942 | 370 |

The control alone shows RSS growth despite nearly constant retained JS heap.
In the toggle arm, DOM nodes/listeners do not keep accumulating after cycle 5,
but retained JS heap still grows about 0.34 MiB and RSS about 7.68 MiB from cycle 5
to 20. This is not a demonstrated plateau. It neither proves a leak nor rules out
a small retained-object leak or native allocation growth. The document counter
is 1 throughout the control; toggling changes it to 2 by cycle 5 and it remains 2.
Its ownership was not traced, so no specific cache/document cause is asserted.

## Interpretation and limits

The original peak-RSS failure and repeated-use leak diagnosis are different
questions. RSS includes the whole browser process tree, and can reflect rendering
storage, allocator reserves, shared mappings and garbage-collection timing.
A larger RSS value alone does not identify which owner retained memory or whether
it is retained unnecessarily. Conversely, a stable DOM count cannot rule out native leaks.

The ranked possibilities were bounded rendering/cache working memory, temporary
objects awaiting collection, and accumulating retained references. These results
distinguish some of their observables but do not identify a single cause. There
is no large continuing DOM-count increase in this 20-cycle experiment; there is
small continuing retained-heap growth and nonzero RSS growth. A longer natural
GC series and retained-object/native allocation profiles would be the next
causal probes. They have not run here. No production fix is justified solely by
this finite experiment.

Fixed control-then-toggle ordering, one browser execution per arm, forced GC,
the short interval and synthetic producer limit extrapolation. This is not
Firefox, engine/broker memory or dedicated-host release-matrix evidence. No
history of passing reruns or this diagnostic converts the failed 32 MiB gate into
a pass. Manual and release performance requirements remain open.

## Artifacts and reproduction

[Manifest](manifest.json) records exact commit, source/binary/corpus hashes,
commands, browser/host, counts, raw hashes and all checkpoint values.
[Original rerun](original-density.tar.gz) preserves the normal Chromium command
and complete raw density result. [Growth diagnostic](growth-raw.tar.gz) preserves
the diagnostic test/config, console log and all 44 snapshots per arm, including
natural pre-GC checkpoints.

Extract the growth archive into this directory, copy
`memory-growth-probe.spec.ts` to `web/e2e/` and
`playwright.memory-growth.config.ts` to `web/`, then run from the repository root:

```sh
VOLMAP_DENSITY_PORT=41744 mise x node@24.19.0 -- corepack pnpm --dir web exec playwright test --config playwright.memory-growth.config.ts --project chromium
```

The temporary files were removed from `web/` afterwards. Production/test/build
inputs remain unchanged, apart from the deliberately separate archived diagnostic;
the unrelated pre-existing producer07 Markdown edit remains excluded. The prior
full-suite pass remains unchanged-source evidence, not a fresh execution.
[Independent review](review.md) records the evidence review, not release approval.
