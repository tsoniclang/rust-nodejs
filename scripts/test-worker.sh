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
loader_registration="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../tsonic-rust" && pwd)/scripts/register-tsonic-root-loader.mjs"
node "${TSONIC_ROOT:-../tsonic}/scripts/certification/capture-tests.mjs" node node \
  --import "$loader_registration" \
  --test --test-reporter=tap --test-concurrency="${TSONIC_TEST_WORKERS}" "${test_files[@]}"
export CARGO_BUILD_JOBS="$TSONIC_TEST_CPUS"
export RUST_TEST_THREADS="$TSONIC_TEST_CPUS"
exec node "${TSONIC_ROOT:-../tsonic}/scripts/certification/capture-tests.mjs" cargo cargo test --locked --workspace
