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
