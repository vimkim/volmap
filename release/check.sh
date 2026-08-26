#!/usr/bin/env bash
set -euo pipefail

readonly TARGET=x86_64-unknown-linux-musl
readonly SOURCE_DATE_EPOCH=1786685990
readonly ABOUT_VERSION='cargo-about 0.9.2'
readonly CYCLONEDX_VERSION='cargo-cyclonedx-cyclonedx 0.5.9'
readonly DENY_VERSION='cargo-deny 0.20.2'
readonly NODE_VERSION='24.19.0'
readonly PNPM_VERSION='11.24.0'

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo 'release audit requires a clean, reviewed commit' >&2
  exit 1
fi

[[ $(cargo about --version) == "$ABOUT_VERSION" ]]
[[ $(cargo cyclonedx --version) == "$CYCLONEDX_VERSION" ]]
[[ $(cargo deny --version) == "$DENY_VERSION" ]]
[[ $(node --version) == "v$NODE_VERSION" ]]
[[ $(corepack pnpm --version) == "$PNPM_VERSION" ]]

release/check-frontend.sh

audit_root=$(mktemp -d /tmp/volmap-release-audit.XXXXXX)
cleanup() {
  chmod -R u+w "$audit_root"
  rm -rf -- "$audit_root"
}
trap cleanup EXIT

mkdir -p \
  "$audit_root/source-a" \
  "$audit_root/source-b" \
  "$audit_root/source-metadata" \
  "$audit_root/cargo-home" \
  "$audit_root/pnpm-store"
git archive HEAD | tar -x -C "$audit_root/source-a"
git archive HEAD | tar -x -C "$audit_root/source-b"
git archive HEAD | tar -x -C "$audit_root/source-metadata"

(
  cd "$audit_root/source-a"
  env VOLMAP_PNPM_STORE_DIR="$audit_root/pnpm-store" \
    release/regenerate-frontend.sh
)
(
  cd "$audit_root/source-b"
  env \
    VOLMAP_PNPM_OFFLINE=1 \
    VOLMAP_PNPM_STORE_DIR="$audit_root/pnpm-store" \
    release/regenerate-frontend.sh
)
for artifact in \
  src/web/generated/frontend.js \
  src/web/generated/frontend.css \
  src/web/generated/manifest.json \
  src/web/generated/runtime-packages.json \
  release/frontend/THIRD_PARTY_NOTICES.txt \
  release/frontend/SBOM.cdx.json \
  release/frontend/BUILD_PROVENANCE.json; do
  cmp "$artifact" "$audit_root/source-a/$artifact"
  cmp "$audit_root/source-a/$artifact" "$audit_root/source-b/$artifact"
done

env \
  LC_ALL=C \
  TZ=UTC \
  cargo deny check

env \
  LC_ALL=C \
  TZ=UTC \
  cargo about generate \
    --locked \
    --all-features \
    --target "$TARGET" \
    --fail \
    --output-file "$audit_root/CARGO_THIRD_PARTY_NOTICES.txt" \
    release/THIRD_PARTY_NOTICES.hbs
node release/frontend/merge-notices.mjs \
  "$audit_root/CARGO_THIRD_PARTY_NOTICES.txt" \
  "$audit_root/source-a/release/frontend/THIRD_PARTY_NOTICES.txt" \
  "$audit_root/THIRD_PARTY_NOTICES.txt"
cmp THIRD_PARTY_NOTICES.txt "$audit_root/THIRD_PARTY_NOTICES.txt"

(
  # cargo-cyclonedx writes beside the manifest, so use a disposable third
  # extraction rather than mutating either reproducibility candidate.
  cd "$audit_root/source-metadata"
  env SOURCE_DATE_EPOCH="$SOURCE_DATE_EPOCH" cargo cyclonedx \
    --format json \
    --spec-version 1.5 \
    --all-features \
    --target "$TARGET" \
    --license-strict \
    --license-accept-named 'MIT/Apache-2.0' \
    --license-accept-named 'Apache-2.0/MIT' \
    --override-filename SBOM.cdx
  sed -i -E \
    -e 's|"bom-ref": "path\+file://[^"]+#(volmap@)?0\.0\.0"|"bom-ref": "pkg:cargo/volmap@0.0.0"|' \
    -e 's|"bom-ref": "path\+file://[^"]+#(volmap@)?0\.0\.0 bin-target-([0-9]+)"|"bom-ref": "pkg:cargo/volmap@0.0.0#target-\2"|' \
    -e 's|"ref": "path\+file://[^"]+#(volmap@)?0\.0\.0"|"ref": "pkg:cargo/volmap@0.0.0"|' \
    -e 's|"purl": "pkg:cargo/volmap@0\.0\.0\?download_url=file://\.(#src/[^"]+)?"|"purl": "pkg:cargo/volmap@0.0.0\1"|' \
    SBOM.cdx.json
)
node release/frontend/merge-sbom.mjs \
  "$audit_root/source-metadata/SBOM.cdx.json" \
  "$audit_root/source-a/release/frontend/SBOM.cdx.json" \
  "$audit_root/SBOM.cdx.json"
cmp SBOM.cdx.json "$audit_root/SBOM.cdx.json"

build_one() {
  local source_dir=$1
  local remap_flags
  remap_flags="-C target-feature=+crt-static --remap-path-prefix=$source_dir=/volmap-src --remap-path-prefix=$audit_root/cargo-home=/cargo-home"
  (
    cd "$source_dir"
    umask 022
    env \
      CARGO_HOME="$audit_root/cargo-home" \
      CARGO_INCREMENTAL=0 \
      LC_ALL=C \
      RUSTFLAGS="$remap_flags" \
      SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH \
      TZ=UTC \
      cargo build --release --locked --target "$TARGET"
  )
}

build_one "$audit_root/source-a"
build_one "$audit_root/source-b"

readonly BIN_A="$audit_root/source-a/target/$TARGET/release/volmap"
readonly BIN_B="$audit_root/source-b/target/$TARGET/release/volmap"
cmp "$BIN_A" "$BIN_B"

file "$BIN_A" | rg -q 'ELF 64-bit.*x86-64.*static-pie linked.*stripped'
! readelf -l "$BIN_A" | rg -q 'INTERP'
! readelf -d "$BIN_A" | rg -q 'NEEDED'
! readelf -V "$BIN_A" | rg -q 'GLIBC_'
ldd "$BIN_A" 2>&1 | rg -q 'statically linked|not a dynamic executable'

env \
  CARGO_HOME="$audit_root/cargo-home" \
  LC_ALL=C \
  SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH \
  TZ=UTC \
  cargo test \
    --manifest-path "$audit_root/source-a/Cargo.toml" \
    --release \
    --locked \
    --all-targets \
    --all-features \
    --target "$TARGET"

sha256sum Cargo.lock web/pnpm-lock.yaml THIRD_PARTY_NOTICES.txt SBOM.cdx.json \
  release/frontend/BUILD_PROVENANCE.json "$BIN_A"
echo 'release audit passed: locked Cargo/frontend metadata, browser gates, notices, SBOM, policy, tests, reproducibility, and static ELF'
