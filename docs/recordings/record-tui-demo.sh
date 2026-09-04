#!/usr/bin/env bash

set -eu

repo_root=$(git rev-parse --show-toplevel)
demo_cast=$(mktemp)
demo_snapshot=$(mktemp -d)
fixture_builder=$(mktemp)
trap 'rm -f "$demo_cast" "$fixture_builder" "$demo_snapshot/fixture" "$demo_snapshot/fixture_vinf"; rmdir "$demo_snapshot"' EXIT

for command_name in asciinema agg expect; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "missing required command: $command_name" >&2
    exit 1
  fi
done

just build-release
rustc +1.97.1 --edition=2024 -Dwarnings \
  "$repo_root/tools/create-smoke-fixture.rs" -o "$fixture_builder"
"$fixture_builder" "$demo_snapshot"

VOLMAP_RECORD_BINARY="$repo_root/target/x86_64-unknown-linux-musl/release/volmap"
VOLMAP_RECORD_EXPECT="$repo_root/docs/recordings/tui-demo.exp"
VOLMAP_RECORD_VINF="$demo_snapshot/fixture_vinf"
VOLMAP_RECORD_VOLUME_ROOT=$demo_snapshot
export VOLMAP_RECORD_BINARY VOLMAP_RECORD_EXPECT VOLMAP_RECORD_VINF VOLMAP_RECORD_VOLUME_ROOT

asciinema rec --quiet --overwrite --cols 120 --rows 36 \
  --env TERM,LANG,VOLMAP_RECORD_BINARY,VOLMAP_RECORD_EXPECT,VOLMAP_RECORD_VINF,VOLMAP_RECORD_VOLUME_ROOT \
  --command 'expect "$VOLMAP_RECORD_EXPECT" "$VOLMAP_RECORD_BINARY" "$VOLMAP_RECORD_VINF" "$VOLMAP_RECORD_VOLUME_ROOT"' \
  "$demo_cast"

agg --quiet --cols 120 --rows 36 --font-size 14 --line-height 1.2 \
  --fps-cap 15 --idle-time-limit 2 --last-frame-duration 2 \
  --speed 1.15 --theme github-dark \
  "$demo_cast" "$repo_root/docs/images/tui-demo.gif"

echo "wrote docs/images/tui-demo.gif"
