#!/usr/bin/env bash
set -euo pipefail

readonly NODE_VERSION='24.19.0'
readonly PNPM_VERSION='11.24.0'

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

check_root=$(mktemp -d /tmp/volmap-frontend-check.XXXXXX)
cleanup() {
  rm -rf -- "$check_root"
}
trap cleanup EXIT

[[ $(<.node-version) == "$NODE_VERSION" ]]
rg -q '"packageManager": "pnpm@11\.24\.0\+sha512-' web/package.json
rg -q '"node": "=24\.19\.0"' web/package.json
rg -q '^lockfileVersion:' web/pnpm-lock.yaml
[[ -x release/regenerate-frontend.sh ]]

[[ $(node --version) == "v$NODE_VERSION" ]]
[[ $(corepack pnpm --version) == "$PNPM_VERSION" ]]

corepack pnpm --dir web install --frozen-lockfile --ignore-scripts
node release/frontend/verify-toolchain.mjs web/toolchain.json
rg -q '"revision": "1234"' release/frontend/BUILD_PROVENANCE.json
rg -q '"revision": "1538"' release/frontend/BUILD_PROVENANCE.json
rg -q '"command": "pnpm audit --audit-level high"' \
  release/frontend/BUILD_PROVENANCE.json
corepack pnpm --dir web run typecheck
corepack pnpm --dir web run test
env VOLMAP_FRONTEND_OUT_DIR="$check_root/generated" \
  corepack pnpm --dir web run build

for asset in frontend.js frontend.css manifest.json runtime-packages.json; do
  cmp "src/web/generated/$asset" "$check_root/generated/$asset"
done

node release/frontend/generate-supply-chain.mjs \
  "$check_root/generated/runtime-packages.json" \
  "$check_root/evidence"
for evidence in THIRD_PARTY_NOTICES.txt SBOM.cdx.json BUILD_PROVENANCE.json; do
  cmp "release/frontend/$evidence" "$check_root/evidence/$evidence"
done
corepack pnpm --dir web audit --audit-level high
corepack pnpm --dir web run test:browser

if rg -n '/home/|/tmp/|file://' \
  src/web/generated \
  release/frontend/THIRD_PARTY_NOTICES.txt \
  release/frontend/SBOM.cdx.json \
  release/frontend/BUILD_PROVENANCE.json; then
  echo 'generated frontend artifacts contain a local path' >&2
  exit 1
fi

mkdir "$check_root/no-node"
for command_name in node corepack pnpm; do
  ln -s /bin/false "$check_root/no-node/$command_name"
done
cargo_bin_dir=$(dirname "$(command -v cargo)")
env PATH="$check_root/no-node:$cargo_bin_dir:/usr/bin:/bin" \
  cargo check --locked
env PATH="$check_root/no-node:$cargo_bin_dir:/usr/bin:/bin" \
  cargo test --lib --locked \
    web::assets::tests::generated_frontend_assets_are_ready_for_the_react_cutover
env PATH="$check_root/no-node:$cargo_bin_dir:/usr/bin:/bin" \
  cargo test --lib --locked \
    notices::tests::canonical_notice_covers_project_authority_and_release_graph

echo 'frontend toolchain and generated assets passed'
