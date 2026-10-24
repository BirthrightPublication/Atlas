#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
task_test_directory="$(mktemp -d /tmp/atlas-e2e.XXXXXXXX)"
task_host_pid=''
cleanup() {
  if [[ -n "$task_host_pid" ]]; then kill "$task_host_pid" 2>/dev/null || true; fi
  rm -rf -- "$task_test_directory"
}
trap cleanup EXIT INT TERM
target/debug/atlas-host --dev --data-dir "$task_test_directory" --port 47832 &
task_host_pid=$!
wait "$task_host_pid"
