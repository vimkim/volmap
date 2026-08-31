Type: grilling
Status: resolved
Blocked by: 01

# Choose the CUBRID target branch and format alignment

## Question

Volmap reads only the feat/oos volume format pinned at `e1e651d`, but the requested landing branch for the inspector is develop. Decide where the inspector lands first — develop, feat/oos, or develop-first-then-merge — and what that means for a working demo: a feat/oos-built `cub_server` serving `demodb` that volmap can point at, versus waiting for a merge. Decide whether the wire contract must tolerate BCB-layout differences between branches (see the branch survey from ticket 01), and which CUBRID base commit the inspector work forks from. This is a user decision (upstream strategy and demo priorities), informed by the ticket 01 facts.

## Comments

2026-08-21, from ticket 01's resolution ([Branch exposure parity](../research/branch-exposure-parity.md)): the branches are at surface parity — develop already has the atomic latch, and one wire contract serves both. So the "must the contract tolerate BCB-layout differences" clause of this question is answered: it need not. What remains is pure strategy: where the producer lands first, the demo path (volmap can only read feat/oos-format volumes today), and the cherry-pick plan (known conflicts limited to the `PRM_LAST_ID` enum tail and the `PAGE_OOS` case in `pgbuf_scan_bcb_table`). Develop-first with a cherry-pick onto feat/oos for the demo is now a low-friction candidate.

## Answer

Resolved with the user, 2026-08-21.

- **Canonical landing branch: develop.** The eventual CUBRID PR targets
  develop, giving the inspector its own review identity instead of riding the
  OOS merge timeline. The user asked for code verification of the enum-shift
  risk before accepting; verified directly: develop `storage_common.h:149-166`
  has `PAGE_AREA`..`PAGE_VACUUM_DATA` = 8..13, while pinned `e1e651d` inserts
  `PAGE_OOS` = 8 (`storage_common.h:159`), shifting those six kinds to 9..14.
  Six of fourteen page kinds differ in raw value across branches — fully
  absorbed by ticket 05's standing constraint that the wire carries a
  semantic page-kind vocabulary and never raw `ptype`, so develop-landing
  carries no compatibility risk.
- **Working/demo branch: fork from exactly `e1e651d`** (volmap's pinned
  format authority), so the demo `cub_server` + `demodb` volumes are
  byte-compatible with what volmap reads. The develop PR is prepared by
  cherry-picking the finished producer onto develop; known conflicts are the
  `PRM_LAST_ID` enum tail, the `PAGE_OOS` case in `pgbuf_scan_bcb_table`'s
  switch, and the page-kind mapping table rows (ticket 01).
- **Wire-contract tolerance clause: closed.** No BCB-layout tolerance is
  needed (ticket 01 proved layout parity); the only branch-awareness anywhere
  is the per-branch page-kind table.
- **Governance rider:** the user confirmed work-tracker item 24 defers its
  cub_server page-buffer-observation dimension to this map.
