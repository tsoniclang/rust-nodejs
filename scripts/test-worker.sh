#!/usr/bin/env bash
set -euo pipefail
npm run build
if (( $# == 0 )); then
  mapfile -d '' -t test_files < <(node "${TSONIC_ROOT:-../tsonic}/test/scripts/node-test-files.mjs" nodejs/test)
else
  test_files=("$@")
fi
if (( ${#test_files[@]} == 0 )); then
  printf 'No Node provider tests were discovered.\n' >&2
  exit 2
fi
node --test --test-concurrency="${TSONIC_TEST_WORKERS}" "${test_files[@]}"
cargo test --locked --workspace
