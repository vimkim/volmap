# Current-commit density confirmation: non-passing

Consumer `cab4a9b` was exercised with the existing release density harness.
The original two-browser command exits 1: Chromium LRU incremental sampled
RSS exceeds the unchanged 32 MiB limit. Firefox passes. Two predetermined
Chromium-only follow-ups both pass, proving verdict variability without
establishing a cause or a repaired gate. All attempts are retained; acceptance
is **non-passing**, not the last command's exit status.

| Attempt | Browser | State RSS increment | LRU RSS increment | Worst enabled p95 | Result |
| --- | --- | ---: | ---: | ---: | --- |
| 01 | Chromium | 26.34 MiB | 34.54 MiB | 47.9 ms | Memory failure |
| 01 | Firefox | 21.40 MiB | 26.78 MiB | 58.0 ms | Local sampled pass |
| 02 | Chromium | 26.23 MiB | 27.26 MiB | 55.1 ms | Local sampled pass |
| 03 | Chromium | 15.04 MiB | 26.16 MiB | 58.9 ms | Local sampled pass |

Every browser execution contains four arms: disabled/enabled State, then
enabled/disabled LRU, 40 inputs per arm, 12,288 actually laid-out cells and
1920×1080 viewport. The report retains browser versions, host, corpus hash,
individual timing/RSS samples, rendered and visible counts. CPU/RSS allowances,
mode workload, process-tree accounting and timing thresholds were not modified.
This is 50 ms sampled summed browser-process RSS, not instantaneous peak capture.
The fixture is a synthetic protocol producer, not the real workload matrix.

## Reproduction and provenance

Original command:

```sh
VOLMAP_DENSITY_PORT=41744 just vite::frontend-density
```

Each of the two follow-ups uses the same harness and fresh release server,
restricted to Chromium:

```sh
VOLMAP_DENSITY_PORT=41744 mise x node@24.19.0 -- corepack pnpm --dir web exec playwright test --config playwright.density.config.ts --project chromium
```

[Manifest](manifest.json) binds the full commit, source/test hashes, built release
binary, raw archive hashes, commands, exit codes and per-browser results. Each
`attempt-0N.tar.gz` preserves `web/test-results` and its log before the next
execution replaces the normal Playwright output. No previous evidence is edited.

The harness records a nonempty dirty-diff hash because the producer07 Markdown
ticket was already modified. There are no dirty runtime, frontend, harness or
build-input files under `src`, `web`, `release`, `tools` or `Cargo.lock`; the
source run is bound to committed input without claiming a wholly clean worktree.
The fresh release server forbids existing-server reuse. It invokes the established
locked release build before serving the fixture.

## What this changes

The preparation manifest previously required a density rerun because two older
fingerprints differed. There is now direct current-source evidence of a failing
Chromium memory case. The old `4f129d8` pass remains historical and neither
substitutes for this result nor proves that the new JavaScript caused it.
The density harness itself is unchanged between `4f129d8` and `cab4a9b`.

The original Chromium LRU enabled/disabled absolute sampled peaks are
721.00/686.45 MiB. Their difference is 34.54 MiB; this is not a JavaScript-heap
measurement or proof of a specific retained allocation. Two additional runs
cannot identify the distribution or establish statistical repeatability.

Diagnosis has a runnable, red-capable command but not yet a deterministic
reproduction. The planned first probe—two unchanged-source repetitions—shows
variability. The next discriminating probe is a controlled old/current bundle
comparison with otherwise identical harness/server/browser inputs. If that does
not explain the difference, the existing native raster/allocation diagnostics
provide the next starting point. The bundle comparison has now run, as recorded below; new allocation profiling
has not run here. No production fix, threshold relaxation, forced GC or inflated
baseline was introduced.

Current-commit density repair/qualification, named manual review and the full
dedicated-host performance campaign remain open. The ordinary `just verify` run
at the preparation checkpoint remains applicable to unchanged runtime/test
sources; it is not rerun or relabelled as a density acceptance pass.

## Bundle-only comparison

A temporary copy of the density test intercepts only `/app.js` and serves either
the `4f129d8` or `cab4a9b` committed generated bundle. Both variants use the same
routing hook, current release server, CSS, browser, workload and thresholds.
The order is current, old, old, current. This is a diagnostic intervention, not
an unmodified-harness acceptance run or a full old-consumer baseline.

| Probe | Bundle | State / LRU RSS increment | Worst enabled p95 | Result |
| --- | --- | ---: | ---: | --- |
| current-01 | cab4a9b | 28.34 / 33.43 MiB | 59.7 ms | Memory failure |
| old-01 | 4f129d8 | 31.82 / 18.19 MiB | 338.4 ms | Timing failure |
| old-02 | 4f129d8 | 26.67 / 36.11 MiB | 57.9 ms | Memory failure |
| current-02 | cab4a9b | 19.27 / 25.54 MiB | 61.5 ms | Diagnostic pass |

The unchanged 12,288-cell / 40-input workload executes in every arm. Both bundles
can exceed the memory ceiling with the same current server: the JavaScript
change is not necessary for this failure. This does not exclude a server-side
change or identify a specific native allocation cause. One older-bundle run also
fails timing. A contemporaneous host snapshot shows other compiler/test processes
and nonzero load; it does not prove that contention caused any measured failure.
A stable isolated measurement environment is needed to improve causal confidence.

[Summary](bundle-probe-summary.json) retains bundle hashes, commands and results;
[bundle-probe.tar.gz](bundle-probe.tar.gz) retains the temporary test/config,
both exact bundles, raw reports, logs and host snapshot. To reproduce, extract
the archive into this directory, copy its test to `web/e2e/` and config to `web/`,
then execute the summary's commands from the repository root. Remove those two
temporary files afterwards. The diagnostic copies were removed after execution;
no production files or normal test configuration changed.

The gate remains non-passing. The failed original run is still the release
candidate's density evidence. Neither a diagnostic pass nor an old-bundle failure
waives the original failure. Further isolated native-allocation investigation,
manual review and dedicated-host release measurements remain outstanding.

[Verification and independent reviews](review.md) report zero evidence defects;
they do not convert the recorded measurement failures into acceptance.
