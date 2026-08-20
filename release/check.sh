#!/usr/bin/env bash
set -euo pipefail

readonly TARGET=x86_64-unknown-linux-musl
readonly SOURCE_DATE_EPOCH=1786685990
readonly ABOUT_VERSION='cargo-about 0.9.2'
readonly CYCLONEDX_VERSION='cargo-cyclonedx-cyclonedx 0.5.9'
readonly DENY_VERSION='cargo-deny 0.20.2'

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo 'release audit requires a clean, reviewed commit' >&2
  exit 1
fi

[[ $(cargo about --version) == "$ABOUT_VERSION" ]]
[[ $(cargo cyclonedx --version) == "$CYCLONEDX_VERSION" ]]
[[ $(cargo deny --version) == "$DENY_VERSION" ]]

audit_root=$(mktemp -d /tmp/volmap-release-audit.XXXXXX)
cleanup() {
  chmod -R u+w "$audit_root"
  rm -rf -- "$audit_root"
}
trap cleanup EXIT

mkdir -p "$audit_root/source-a" "$audit_root/source-b" "$audit_root/cargo-home"
git archive HEAD | tar -x -C "$audit_root/source-a"
git archive HEAD | tar -x -C "$audit_root/source-b"

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
    --output-file "$audit_root/THIRD_PARTY_NOTICES.txt" \
    release/THIRD_PARTY_NOTICES.hbs
cmp THIRD_PARTY_NOTICES.txt "$audit_root/THIRD_PARTY_NOTICES.txt"

(
  cd "$audit_root/source-a"
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
cmp SBOM.cdx.json "$audit_root/source-a/SBOM.cdx.json"

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

sha256sum Cargo.lock THIRD_PARTY_NOTICES.txt SBOM.cdx.json "$BIN_A"
echo 'release audit passed: locked metadata, notices, SBOM, policy, tests, reproducibility, and static ELF'
