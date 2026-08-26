# Frontend release evidence

`THIRD_PARTY_NOTICES.txt` and `SBOM.cdx.json` describe only packages whose code
Vite placed in the committed production bundle. `BUILD_PROVENANCE.json`
describes the complete installed Linux x86-64 build graph, including lifecycle
scripts that the immutable pnpm install deliberately did not execute.

Regenerate these files together with the bundle through
`release/regenerate-frontend.sh`. Verify them through
`release/check-frontend.sh`; ordinary Cargo builds consume the committed assets
without running Node or pnpm.
