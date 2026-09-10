#!/usr/bin/env bash
set -euo pipefail

readonly PORT=${VOLMAP_BROWSER_PORT:-41739}
readonly TARGET=x86_64-unknown-linux-musl

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

server_root=$(mktemp -d /tmp/volmap-browser-server.XXXXXX)
server_pid=''
producer_pid=''
cleanup() {
  if [[ -n $producer_pid ]] && kill -0 "$producer_pid" 2>/dev/null; then
    kill "$producer_pid"
    wait "$producer_pid" 2>/dev/null || true
  fi
  if [[ -n $server_pid ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid"
    wait "$server_pid" 2>/dev/null || true
  fi
  if [[ -n ${VOLMAP_BROWSER_PRODUCER_PID_FILE:-} ]]; then rm -f -- "$VOLMAP_BROWSER_PRODUCER_PID_FILE"; fi
  rm -rf -- "$server_root"
}
trap cleanup EXIT INT TERM

rustc --edition=2024 -Dwarnings tools/create-smoke-fixture.rs \
  -o "$server_root/create-smoke-fixture"
mkdir "$server_root/snapshot"
fixture_arguments=()
if [[ ${VOLMAP_BROWSER_DENSE:-0} == 1 ]]; then fixture_arguments=(--dense); fi
"$server_root/create-smoke-fixture" "$server_root/snapshot" "${fixture_arguments[@]}"

build_arguments=()
profile=debug
if [[ ${VOLMAP_BROWSER_RELEASE:-0} == 1 ]]; then
  build_arguments=(--release)
  profile=release
fi
cargo build --locked "${build_arguments[@]}"
runtime_arguments=()
if [[ ${VOLMAP_BROWSER_RUNTIME:-0} == 1 ]]; then
  runtime_arguments=(--runtime-page-buffer --runtime-socket "$server_root/no-producer.sock")
fi
follow_arguments=(--no-follow)
if [[ ${VOLMAP_BROWSER_PRODUCER:-0} == 1 ]]; then
  mkfifo "$server_root/producer-ready"
  python3 -u web/e2e/producer-fixture.py "$server_root/producer.sock" \
    "$server_root/snapshot/fixture" \
    fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl \
    > "$server_root/producer-ready" &
  producer_pid=$!
  if [[ -n ${VOLMAP_BROWSER_PRODUCER_PID_FILE:-} ]]; then printf '%s\n' "$producer_pid" > "$VOLMAP_BROWSER_PRODUCER_PID_FILE"; fi
  read -r producer_ready < "$server_root/producer-ready"
  [[ $producer_ready == ready ]]
  runtime_arguments=(--runtime-page-buffer --runtime-socket "$server_root/producer.sock")
  follow_arguments=()
fi
"target/$TARGET/$profile/volmap" serve \
  --vinf "$server_root/snapshot/fixture_vinf" \
  --volume-root "$server_root/snapshot" \
  --listen "127.0.0.1:$PORT" \
  "${follow_arguments[@]}" \
  "${runtime_arguments[@]}" \
  --progress never &
server_pid=$!
wait "$server_pid"
