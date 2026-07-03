#!/usr/bin/env bash
# Packages the tsonic_rust_node runtime crate into runtimes/crates/ as the
# committed artifact shipped with the @tsonic/rust-nodejs npm package.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_dir="$repo_root/rust/crates/tsonic_rust_node"
packaged_dir="$repo_root/runtimes/crates/tsonic_rust_node"

mkdir -p "$packaged_dir"
rsync -a --delete --exclude "target/" "$source_dir/" "$packaged_dir/"
