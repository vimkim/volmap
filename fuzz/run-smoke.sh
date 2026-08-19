#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

readonly FUZZ_TOOLCHAIN=nightly-2026-08-18
runs=${VOLMAP_FUZZ_RUNS:-1000}
corpus_root=$(mktemp -d /tmp/volmap-fuzz-corpus.XXXXXX)
cleanup() {
  rm -rf -- "$corpus_root"
}
trap cleanup EXIT

for target in byte_access page_envelope volume_bitmap slotted_records metadata_pages tde_key_info; do
  mkdir "$corpus_root/$target"
done

cp fixtures/e1e651de/pages/vol0-page0.bin "$corpus_root/page_envelope/volume-header"
cp fixtures/e1e651de/pages/vol0-page0.bin "$corpus_root/volume_bitmap/header-bitmap"
dd if=fixtures/e1e651de/pages/vol0-page1.bin \
  of="$corpus_root/volume_bitmap/header-bitmap" bs=16384 seek=1 conv=notrunc status=none
for page in vol1-page642.bin vol1-page705.bin vol1-page770.bin vol1-page771.bin vol1-page772.bin; do
  cp "fixtures/e1e651de/pages/$page" "$corpus_root/slotted_records/$page"
done
for page in vol0-page64.bin vol0-page641.bin vol0-page705.bin vol1-page640.bin vol1-page960.bin vol1-page961.bin vol1-page962.bin; do
  cp "fixtures/e1e651de/pages/$page" "$corpus_root/metadata_pages/$page"
done
cp fixtures/e1e651de/pages/vol0-page321.bin "$corpus_root/slotted_records/vol0-page321.bin"
cp fixtures/e1e651de/pages/vol0-page577.bin "$corpus_root/slotted_records/vol0-page577.bin"

for target in byte_access page_envelope volume_bitmap slotted_records metadata_pages tde_key_info; do
  cargo +"$FUZZ_TOOLCHAIN" fuzz run "$target" "$corpus_root/$target" -- \
    -runs="$runs" -max_len=32768 -timeout=5
done
