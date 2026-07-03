#!/usr/bin/env bash
# Packages the tsonic_rust_node runtime crate into runtimes/crates/ as the
# committed artifact shipped with the @tsonic/rust-nodejs npm package.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_dir="$repo_root/rust/crates/tsonic_rust_node"
packaged_dir="$repo_root/runtimes/crates/tsonic_rust_node"

mkdir -p "$packaged_dir"
rsync -a --delete --exclude "target/" "$source_dir/" "$packaged_dir/"

# The packaged crate is a lib-only dependency: strip repo-relative [[test]]
# target sections that do not ship with the package.
awk '
  /^\[\[test\]\]$/ { skip = 1; next }
  /^\[/ && !/^\[\[test\]\]$/ { skip = 0 }
  !skip { print }
' "$packaged_dir/Cargo.toml" | cat -s > "$packaged_dir/Cargo.toml.tmp"
mv "$packaged_dir/Cargo.toml.tmp" "$packaged_dir/Cargo.toml"
