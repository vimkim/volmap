# Reassess the browser overlay memory budget

Status: revised policy; release qualification remains open.

On 2026-09-11 the user authorized reassessment and confirmed ordinary developer
PCs with 1–8 tabs as the primary usage. Within that work, the implementation
selects **64 MiB incremental peak RSS per browser tab** as the revised allowance.
The user confirmed the usage profile; the numeric choice is an engineering
policy decision, not a user-selected value or measured sufficiency claim.
Historical results retain their original thresholds and verdicts.

## Decision and rationale

Use 64 MiB instead of 32 MiB for the browser allowance. The maximum ordinary
current-source density observation below is 34.54 MiB: the new ceiling provides
about 1.85 times that measurement (29.46 MiB headroom). This is a deliberately
rounded product allowance, not a statistical confidence bound. At eight tabs,
64 MiB each represents a nominal 512 MiB incremental allowance, separate from
the browser's ordinary inspection footprint. Summed process RSS includes shared
mappings, so this arithmetic is not a prediction of physical RAM consumption.

This modest increase accommodates the observed ordinary case without jumping
to 128 MiB per tab (a nominal 1 GiB over eight tabs) without stronger evidence.
It is **not** a fix for continued retained-memory growth. The repeated-use
snapshot difference below already exceeds 64 MiB under a different method;
that uncertainty remains open and must be measured with natural GC and matched
workloads before release qualification. A failing new run remains a failure;
there is no automatic increase or retry-until-pass policy.

Primary usage does not delete the existing 1/8/32-tab release matrix or change
producer/broker allocation limits, timing, accessibility, or disclosure rules.
The same browser policy applies to the future 64-sector scope, whose expanded
workload still requires its own evidence. The revision supersedes only the
browser RSS number in the original planning decision.

## Why reconsider

The [original decision](../pgbuf-overlay/issues/15-set-overlay-resource-budgets.md)
labels the numbers provisional design limits and future measurement gates.
It records acceptance of the 32 MiB/browser-tab recommendation, but supplies no
measurement or derivation showing that 32 MiB is necessary. This differs from
protocol/allocation bounds whose enforcement protects bounded decoding.

The browser budget measures incremental RSS relative to a matched disabled
workload, not total browser memory. It also does not determine whether memory is
leaking. A useful replacement must state both the tolerated memory cost and the
workloads/tab counts to which the measurement applies.

## Available measurements

Comparable ordinary density runs on the unchanged current runtime sources:

| Run | Browser | State increment | LRU increment |
| --- | --- | ---: | ---: |
| Candidate attempt 01 | Chromium | 26.34 MiB | 34.54 MiB |
| Candidate attempt 01 | Firefox | 21.40 MiB | 26.78 MiB |
| Candidate attempt 02 | Chromium | 26.23 MiB | 27.26 MiB |
| Candidate attempt 03 | Chromium | 15.04 MiB | 26.16 MiB |
| Later original-harness repeat | Chromium | 16.98 MiB | 21.47 MiB |

Sources: [candidate raw attempts](verification/08-candidate-density/README.md)
and [later original rerun](verification/08-memory-growth/README.md). These are
one-tab synthetic 12,288-cell cases with 40 inputs per arm and 50 ms sampled summed
browser-process RSS. They show that 32 MiB can be marginal for this case; they do
not establish real-engine, resident-heavy, repeated-use or multi-tab budgets.
The old/current bundle-interception probes are diagnostic and excluded from this
table.

Repeated-use characterization is a separate warning against choosing a new
number from the first-enable table alone. At cycle 20, post-GC RSS snapshots were
715.56 MiB in the toggle arm and 648.90 MiB in the disabled control, a 66.66 MiB
difference. These fixed-order, separate-browser snapshots with forced GC are
not matched peak-RSS acceptance evidence. They neither prove that 64 MiB is
insufficient under the proper release protocol nor establish a need for 128 MiB.
They identify repeated-use measurements that a replacement budget must examine.

## Qualification and implementation

The active implementation spec, ticket 06/08 requirements, release guide and
density test use 64 MiB. Test reports record the actual byte ceiling as well as
the measured increments so later policy changes cannot obscure a run's contract.
The future 64-sector spec links this revision. Historical planning decisions,
executed manifests and archives remain unchanged; their 32 MiB verdicts still
mean what they meant when executed.

Rerun the original two-browser density case once with this recorded policy and
retain its raw outputs under `verification/08-browser-budget-revision/`.
These local one-tab synthetic checks cannot establish the 1–8-tab usage target.
Qualification still requires representative real-engine workloads, matched
disabled tab counts and repeated enable/mode/navigation cycles with natural GC.
Record settled/retained growth separately from incremental peak RSS.

The dedicated-host matrix, manual accessibility and other release gates remain
open until their actual evidence exists. This revision is not release approval.
