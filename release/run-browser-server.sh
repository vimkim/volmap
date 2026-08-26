#!/usr/bin/env bash
set -euo pipefail

readonly PORT=${VOLMAP_BROWSER_PORT:-41739}
readonly TARGET=x86_64-unknown-linux-musl

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

server_root=$(mktemp -d /tmp/volmap-browser-server.XXXXXX)
server_pid=''
cleanup() {
  if [[ -n $server_pid ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid"
    wait "$server_pid" 2>/dev/null || true
  fi
  rm -rf -- "$server_root"
}
trap cleanup EXIT INT TERM

rustc --edition=2024 -Dwarnings tools/create-smoke-fixture.rs \
  -o "$server_root/create-smoke-fixture"
mkdir "$server_root/snapshot"
"$server_root/create-smoke-fixture" "$server_root/snapshot"

cargo build --locked
"target/$TARGET/debug/volmap" serve \
  --vinf "$server_root/snapshot/fixture_vinf" \
  --volume-root "$server_root/snapshot" \
  --listen "127.0.0.1:$PORT" \
  --no-follow \
  --progress never &
server_pid=$!
wait "$server_pid"
