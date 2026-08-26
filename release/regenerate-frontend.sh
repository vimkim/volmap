#!/usr/bin/env bash
set -euo pipefail

readonly NODE_VERSION='24.19.0'
readonly PNPM_VERSION='11.24.0'

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

[[ $(node --version) == "v$NODE_VERSION" ]]
[[ $(corepack pnpm --version) == "$PNPM_VERSION" ]]

install_arguments=(--frozen-lockfile --ignore-scripts)
if [[ -n ${VOLMAP_PNPM_STORE_DIR:-} ]]; then
  install_arguments+=(--store-dir "$VOLMAP_PNPM_STORE_DIR")
fi
if [[ ${VOLMAP_PNPM_OFFLINE:-0} == 1 ]]; then
  install_arguments+=(--offline)
fi
corepack pnpm --dir web install "${install_arguments[@]}"
node release/frontend/verify-toolchain.mjs web/toolchain.json
corepack pnpm --dir web run typecheck
corepack pnpm --dir web run test
corepack pnpm --dir web run build
node release/frontend/generate-supply-chain.mjs \
  src/web/generated/runtime-packages.json \
  release/frontend

if rg -n '/home/|/tmp/|file://' \
  src/web/generated \
  release/frontend/THIRD_PARTY_NOTICES.txt \
  release/frontend/SBOM.cdx.json \
  release/frontend/BUILD_PROVENANCE.json; then
  echo 'generated frontend artifacts contain a local path' >&2
  exit 1
fi

sha256sum \
  web/pnpm-lock.yaml \
  src/web/generated/frontend.js \
  src/web/generated/frontend.css \
  src/web/generated/manifest.json \
  src/web/generated/runtime-packages.json \
  release/frontend/THIRD_PARTY_NOTICES.txt \
  release/frontend/SBOM.cdx.json \
  release/frontend/BUILD_PROVENANCE.json
