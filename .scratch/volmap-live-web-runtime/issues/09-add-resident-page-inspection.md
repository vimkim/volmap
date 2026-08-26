# 09: Add selected resident-page inspection and correspondence

**What to build:** Add W8's explicit selected-page resident structure request and truthful disk/resident comparison. This operation remains separate from state-only polling.

**Blocked by:** 04: Add attribute selection and cross-highlighting; 08: Consume CUBRID state-only page-buffer observations.

**Status:** ready-for-agent

- [ ] Resident inspection occurs only after an explicit selected-page action and never as a side effect of volume/sector polling.
- [ ] The result contains sanitized semantic slotted-page structure, capture identity, per-field limitations, and no raw bytes, values, private structs, addresses, holder identities, or reconstructed event history.
- [ ] Disk generation and resident capture remain separately labelled with independent capture times and provenance.
- [ ] Relation is exactly matching, divergent, or unknown and is evidence-backed for the displayed disk generation plus exact resident capture token.
- [ ] Only matching allows one geometry to cross-highlight disk and resident views; divergent and unknown remain side-by-side while disk-derived attribute highlighting remains valid.
- [ ] Disk generation advance, route change, producer restart, incarnation change, or late scope mismatch clears resident correspondence before rendering.
- [ ] A state-only batch may repopulate after disk advance but cannot recreate resident structure or correspondence.
- [ ] Browser/model tests cover match, divergence, unknown, restart, disk advance, route race, late response, unsupported page type, and structural disclosure.
