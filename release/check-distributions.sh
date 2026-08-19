#!/usr/bin/env bash
set -euo pipefail

readonly TARGET=x86_64-unknown-linux-musl
readonly DEBIAN_IMAGE='docker.io/library/debian@sha256:38a76d01668772e381ad2826d876627c89e7133e2f8a0f5d567306798b0f2a16'
readonly ROCKY_IMAGE='docker.io/library/rockylinux@sha256:d644d203142cd5b54ad2a83a203e1dee68af2229f8fe32f52a30c6e1d3c3a9e0'
readonly ALPINE_IMAGE='docker.io/library/alpine@sha256:79ff19e9084a00eece421b2523fb93e22d730e2c0e525905de047e848e56d95f'

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

binary=${1:-target/$TARGET/release/volmap}
binary=$(realpath "$binary")
[[ -x $binary ]]
file "$binary" | rg -q 'ELF 64-bit.*x86-64.*static-pie linked.*stripped'

audit_root=$(mktemp -d /tmp/volmap-distribution-audit.XXXXXX)
cleanup() {
  chmod -R u+w "$audit_root"
  rm -rf -- "$audit_root"
}
trap cleanup EXIT

rustc --edition=2024 -Dwarnings tools/create-smoke-fixture.rs \
  -o "$audit_root/create-smoke-fixture"
mkdir "$audit_root/snapshot"
"$audit_root/create-smoke-fixture" "$audit_root/snapshot"

run_case() {
  local name=$1
  local image=$2
  local output="$audit_root/$name"
  mkdir "$output"
  podman image exists "$image" || podman pull "$image"
  [[ $(podman image inspect --format '{{.Architecture}}/{{.Os}}' "$image") == amd64/linux ]]

  local -a container=(
    podman run --rm --network none --read-only --security-opt label=disable
    -v "$binary:/volmap:ro"
    -v "$audit_root/snapshot:/snapshot:ro"
    -v "$output:/output:rw"
    "$image"
  )
  "${container[@]}" /volmap --version >"$output/version.txt"
  "${container[@]}" /volmap licenses --format json >"$output/licenses.json"
  "${container[@]}" /volmap summary --vinf /snapshot/fixture_vinf \
    --format json --progress never >"$output/summary.json"
  "${container[@]}" /volmap map --vinf /snapshot/fixture_vinf volume:0 \
    --format jsonl --progress never >"$output/map.jsonl"
  "${container[@]}" /volmap inspect --vinf /snapshot/fixture_vinf page:0:10 \
    --format json --progress never >"$output/page.json"
  "${container[@]}" /volmap export html --vinf /snapshot/fixture_vinf \
    --output /output/report.html --enrich page:0:10 --progress never
}

run_case debian "$DEBIAN_IMAGE"
run_case rocky "$ROCKY_IMAGE"
run_case alpine "$ALPINE_IMAGE"

for artifact in version.txt licenses.json summary.json map.jsonl page.json report.html; do
  cmp "$audit_root/debian/$artifact" "$audit_root/rocky/$artifact"
  cmp "$audit_root/debian/$artifact" "$audit_root/alpine/$artifact"
done

sha256sum "$binary" "$audit_root/debian/summary.json" \
  "$audit_root/debian/map.jsonl" "$audit_root/debian/page.json" \
  "$audit_root/debian/report.html"
echo 'distribution audit passed: Debian 13, Rocky 9, and Alpine 3.24'
