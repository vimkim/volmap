# Follow-up memory diagnosis — non-passing

Consumer code remains `8810d047256d70d17a8ceb204f1e3cd756062105`,
checked out through evidence commit `951fffd`. No production changes resulted
from this investigation. The original acceptance record remains authoritative.

The original Chromium density command failed again. `revalidation.tar.gz`
preserves its full report and log. A smaller Chromium-only reproduction retained
one LRU enabled/disabled pair, 12,288 rendered cells and 40 Tab inputs per arm;
it also failed the unchanged 32 MiB gate. Probe runs used the same local release
server, corpus and host as the original run.

Three hypotheses were investigated: accessibility-role setup queries materially
inflate the difference; live overlay heap/DOM retention explains it; or memory
outside the live JavaScript heap contributes. These are diagnostic hypotheses,
not established root causes.

`phase-baseline.tar.gz` adds Chromium CDP performance/DOM counters after loading,
after enabling/selecting LRU, and after the inputs. `css-selectors.tar.gz` changes
only the setup/focus selectors to equivalent CSS selectors. It still fails:
ending sampled RSS is 753.07 MiB enabled versus 712.70 MiB disabled. Therefore
removing accessibility-role queries does not by itself satisfy the gate. Separate
fresh browser processes and run-to-run variation prevent interpreting differences
between these individual runs as a precise allocation effect.

`forced-gc.tar.gz` then adds forced garbage collection after the inputs solely as
a diagnostic. Enabled RSS falls from 758.70 to 691.13 MiB; disabled RSS falls from
712.67 to 656.23 MiB. Post-GC live heap is 21.33 versus 13.10 MiB, and DOM node
counts are 26,850 versus 26,224. Reclaimable allocation contributes to the samples,
but the remaining RSS difference is about 34.90 MiB. This does not prove a leak,
a specific renderer defect or a passing memory gate. Forced GC is not part of
the acceptance harness, and no post-GC metric replaces peak RSS.

[Phase values](phase-summary.json) retain exact numbers. Each probe archive
contains its temporary TypeScript test/configuration, original run log and raw
per-arm samples with provenance. The probes were removed from the active test
suite after diagnosis. To reproduce a probe, restore its test as
`web/e2e/memory-probe.spec.ts` and config as
`web/playwright.memory-probe.config.ts`, then run:

```sh
mise x node@24.19.0 -- corepack pnpm --dir web exec playwright test --config playwright.memory-probe.config.ts
```

The probe instruments Chromium, tests only one mode/pair and alters timing with
CDP measurements. It is not release qualification. An initial CSS probe used an
incorrect selector and was interrupted; the archived CSS run uses the correct
selector and completes both arms. The reference-host designation, qualified
peak-memory evidence and named manual screen-reader/visual reviews remain open.
Archive integrity hashes are recorded in `sha256.json`.
