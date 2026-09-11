# 07: Port to develop and hand the pin to Volmap

**What to build:** The Volmap owner receives a corpus aggregate hash that is identical on the format-aligned branch and on a develop-based branch, both producer commits, captured test execution evidence from both, and verification instructions in standard project terms, so Volmap ticket 02 can vendor the corpus and verify it offline. The contract has been reviewed against every cited decision before handoff.

**Blocked by:** 01, 02, 03, 04, 05, 06.

**Status:** ready-for-agent

- [ ] The finished artifacts are cherry-picked onto a develop-based branch in a second fresh worktree. The only source differences are the page-kind mapping's out-of-row enumerator and unit-test registration drift. Mapping tests on develop assert that `oos` is never produced and that every develop enumerator maps to a name.
- [ ] The full unit-test module executes on both branches with captured case listings and summary counts. The checksum file and manifest are byte-identical on both branches and the aggregate hash matches on both; a differing hash is a defect to fix, not to document.
- [ ] The contract document is reviewed line by line against the transport, gating, wire v1, security, verification, exact-LRU-membership and budget decisions and against the consumer specification. Each of the ten accepted concrete-syntax clarifications is marked confirmed. Any deviation found is resolved by amending the relevant decision ticket, never by adjusting syntax silently or adding consumer exceptions.
- [ ] A handoff record for Volmap states the contract version and corpus revision, the aggregate hash, both producer commits as provenance, the vendoring procedure, the execution evidence for both branches, and the explicit statement that no engine producer, socket or runtime behavior exists yet. It contains no personal tooling, workspace paths or claims of hosted CI.
- [ ] Nothing is pushed, no PR is opened and JIRA is unchanged. Volmap ticket 02's external prerequisite is reported satisfied only after the Volmap owner confirms the vendored aggregate hash offline.
