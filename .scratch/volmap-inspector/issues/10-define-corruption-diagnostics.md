Type: grilling
Status: open
Blocked by: 02, 04

# Define corruption containment and diagnostic semantics

## Question

When volume, file-table, page, slot, or OOS-chain bytes violate the pinned format, exactly what remains trustworthy and what must stop? Define validation boundaries, diagnostic identities and severity, safe arithmetic and bounds rules, cycle and overlap detection, per-volume/page containment, incomplete-report markers, nonzero exit behavior, and UI/JSON representation. The result must operationalize the standing rule: continue only where boundaries remain independently trustworthy and never infer across an untrusted offset or length.

## Comments
