Type: grilling
Status: resolved
Blocked by: 04, 05

# Choose the consistency-inspection boundary

## Question

The state-only v1 producer is now release-capable, while protected page-image
capture, memory/main-volume/DWB digests, TDE normalization, optimistic A/B
capture bracketing, and consistency classification are a separate debug-only
capability. Decide whether this Wayfinder map carries that capability to an
implementation-ready contract or stops at the state-only runtime overlay and
moves consistency inspection into a later map.

If it remains here, specify the point-operation boundary, evidence and
classification vocabulary, capability negotiation, resource limits, and its
relationship to Volmap's observed disk state. If it moves out, transfer the
corresponding design-reference extensions and DWB/disk cross-check dependencies
out of this map without weakening the state-only v1 contract.

## Comments

## Answer

Stop this map at the implementation-ready **state-only runtime observation
overlay**. Protected resident-page capture and persistence comparison form a
separate **resident page inspection** capability and move to a later Wayfinder
map.

The current map retains the release-capable, default-off state-only producer
and its bulk resident-set scan. That contract remains deliberately limited to
semantic BCB evidence: it does not copy page images, read persistent images,
probe the DWB, normalize TDE content, compute memory or disk digests, or issue a
consistency classification. `InspectPage(VPID)` remains deferred; omission from
a complete resident-set scan means only `NOT_RESIDENT`, as already defined by
[Define wire contract v1](05-define-wire-contract-v1.md).

The later consistency-inspection map must decide the targeted point-operation
contract, protected A/B capture bracket, main-volume and DWB evidence, TDE
normalization, digest and classification vocabulary, capability negotiation,
resource and latch-wait limits, and the relationship between runtime evidence
and Volmap's observed disk state. The optional independent Volmap disk
cross-check belongs there as a cross-check, not as the primary classification
oracle.

AOUT/recent-eviction observation and a transition changefeed do not capture or
compare page images, so this decision does not move them into the consistency
effort; they remain state-observation fog for this map until separately scoped.
The existing glossary already distinguishes **page-buffer observation**,
**resident page inspection**, and **page image correspondence**, so this
decision requires no new domain term.
