#!/usr/bin/env bash
set -euo pipefail

readonly TARGET=x86_64-unknown-linux-musl
readonly SOURCE_DATE_EPOCH=1786685990
readonly ABOUT_VERSION='cargo-about 0.9.2'
readonly CYCLONEDX_VERSION='cargo-cyclonedx-cyclonedx 0.5.9'

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

generation_root=$(mktemp -d /tmp/volmap-release-generation.XXXXXX)
cleanup() {
  rm -rf -- "$generation_root"
}
trap cleanup EXIT

[[ $(cargo about --version) == "$ABOUT_VERSION" ]]
[[ $(cargo cyclonedx --version) == "$CYCLONEDX_VERSION" ]]

release/regenerate-frontend.sh

cargo about generate \
  --locked \
  --all-features \
  --target "$TARGET" \
  --fail \
  --output-file "$generation_root/CARGO_THIRD_PARTY_NOTICES.txt" \
  release/THIRD_PARTY_NOTICES.hbs

node release/frontend/merge-notices.mjs \
  "$generation_root/CARGO_THIRD_PARTY_NOTICES.txt" \
  release/frontend/THIRD_PARTY_NOTICES.txt \
  THIRD_PARTY_NOTICES.txt

env SOURCE_DATE_EPOCH="$SOURCE_DATE_EPOCH" cargo cyclonedx \
  --format json \
  --spec-version 1.5 \
  --all-features \
  --target "$TARGET" \
  --license-strict \
  --license-accept-named 'MIT/Apache-2.0' \
  --license-accept-named 'Apache-2.0/MIT' \
  --override-filename SBOM.cdx

# cargo-cyclonedx encodes the checkout path for a local root package. Replace
# only that root identity with its canonical package URL so the SBOM is both
# relocatable and reproducible across the mandatory independent checkouts.
sed -i -E \
  -e 's|"bom-ref": "path\+file://[^"]+#(volmap@)?0\.0\.0"|"bom-ref": "pkg:cargo/volmap@0.0.0"|' \
  -e 's|"bom-ref": "path\+file://[^"]+#(volmap@)?0\.0\.0 bin-target-([0-9]+)"|"bom-ref": "pkg:cargo/volmap@0.0.0#target-\2"|' \
  -e 's|"ref": "path\+file://[^"]+#(volmap@)?0\.0\.0"|"ref": "pkg:cargo/volmap@0.0.0"|' \
  -e 's|"purl": "pkg:cargo/volmap@0\.0\.0\?download_url=file://\.(#src/[^"]+)?"|"purl": "pkg:cargo/volmap@0.0.0\1"|' \
  SBOM.cdx.json

node release/frontend/merge-sbom.mjs \
  SBOM.cdx.json \
  release/frontend/SBOM.cdx.json \
  "$generation_root/SBOM.cdx.json"
mv "$generation_root/SBOM.cdx.json" SBOM.cdx.json

if rg -n 'path\+file:|download_url=file:|/home/|/tmp/' \
  THIRD_PARTY_NOTICES.txt SBOM.cdx.json release/frontend/BUILD_PROVENANCE.json; then
  echo 'generated release metadata contains a local path' >&2
  exit 1
fi

sha256sum Cargo.lock web/pnpm-lock.yaml THIRD_PARTY_NOTICES.txt SBOM.cdx.json \
  release/frontend/BUILD_PROVENANCE.json
