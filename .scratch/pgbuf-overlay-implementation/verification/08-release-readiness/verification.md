# Preparation verification

Scope: documentation, gate index and preserved-evidence inventory on base
`08ed9477bd7414c6a320a2c06ee7b439e8685ba5`. No runtime code, generated frontend,
producer or test behavior changes. No new implementation seam requires TDD.

JSON and link validation passed: 29 required gate rows, 50 inherited contract
entries, 126 inventory SHA-256 matches and four original density archive hash
matches. All six 06 and four 07 source fingerprints were compared; mismatches are
recorded as rerun requirements rather than accepted evidence. Relative links in
the edited operator/verification/protocol documents and handoff resolve.

Recheck the inventory from the repository root:

```sh
python3 - <<'PY'
import hashlib, json
from pathlib import Path
root = Path('.scratch/pgbuf-overlay-implementation/verification/08-release-readiness')
manifest = json.loads((root / 'manifest.json').read_text())
assert manifest['release_ready'] is False
assert manifest['performance_matrix']['executed_count'] == 0
inventory = json.loads((root / 'evidence-inventory.json').read_text())
for entry in inventory['files']:
    assert hashlib.sha256(Path(entry['path']).read_bytes()).hexdigest() == entry['sha256'], entry['path']
print(f"Verified {len(inventory['files'])} retained file hashes; release readiness remains open")
PY
```

Local regression result and independent Standards/Spec review are appended below.

## Local regression

`just verify` exited 0. Rust tests, Clippy, static-musl release checks, frontend
typechecking, 74 Vitest cases, advisories, generated-artifact checks and Playwright
all passed. Browser result: 65 passed, one existing Firefox parity skip owned by
Chromium. Existing three manual Rust ignores remain; no release performance gate
is counted as executed. See [raw output](local-verify.log.gz).

The browser suite rewrites tracked screenshot evidence as a side effect. Those
outputs were restored to their pre-run committed bytes, preserving historical
records; the full raw execution log remains here. The pre-existing producer07
ticket edit is excluded. `git diff --check` passes.

## Standards

Independent review of `08ed947...86d7f35`: no findings. Local-ticket conventions,
glossary, read-only guarantees and retained evidence are preserved. ADR 0006's
revised listener decision and ADR 0008's unverified implementation scope are
explicit. Manual review, performance execution, candidate reruns and final
acceptance remain open. Reviewer independently checked all 126 inventory hashes
and the local browser result. Documented violations: 0; baseline smells: 0.

## Spec

Independent review of the same nonempty one-commit diff: no actionable preparation
defects. All 126 inventory hashes, both inherited manifest hashes and the four
accepted 07 reports (12 passing cases each, zero failures/skips) were checked.
Dirty-base provenance, missing manual reviews and changed-source qualifications
are retained. Thresholds are unchanged and no measurements are fabricated.
Operator documentation and existing-ADR disclosure stay within scope.

Dedicated-host measurements, manual reviews, candidate reruns and final named
per-invariant mapping are expected remaining release requirements, not completed
by this preparation. Aggregate inherited contract mapping is explicitly
provisional. Scope creep: 0; actionable incorrect implementation findings: 0.

Standards: 0 findings. Spec: 0 actionable preparation findings; release gates stay
open. The subsequent documentation-only closeout records this review and marks
only the two completed operator/limitation documentation checklist items.
