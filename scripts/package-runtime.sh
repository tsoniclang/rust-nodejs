#!/usr/bin/env bash
# Packages the tsonic_rust_node runtime crate into runtimes/crates/ as the
# committed artifact shipped with the @tsonic/rust-nodejs npm package.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_dir="$repo_root/rust/crates/tsonic_rust_node"
packaged_dir="$repo_root/runtimes/crates/tsonic_rust_node"

mkdir -p "$packaged_dir"
rsync -a --delete --exclude "target/" --exclude ".temp/" "$source_dir/" "$packaged_dir/"

# The packaged crate is a lib-only dependency: strip repo-relative [[test]]
# target sections that do not ship with the package.
awk '
  /^\[\[test\]\]$/ { skip = 1; next }
  /^\[/ && !/^\[\[test\]\]$/ { skip = 0 }
  !skip { print }
' "$packaged_dir/Cargo.toml" | cat -s > "$packaged_dir/Cargo.toml.tmp"
mv "$packaged_dir/Cargo.toml.tmp" "$packaged_dir/Cargo.toml"

# Rewrite sibling-repo path dependencies to the flat node_modules peer layout:
# inside an installed package the crate lives at
#   node_modules/@tsonic/rust-nodejs/runtimes/crates/tsonic_rust_node
# and the @tsonic/target-rust peer (which ships the runtime crates under
# runtimes/crates/) is four levels up at node_modules/@tsonic/target-rust.
sed -i \
  -e 's|{ path = "../../../../rust-js/crates/tsonic_rust_js" }|{ path = "../../../../target-rust/runtimes/crates/tsonic_rust_js" }|' \
  -e 's|{ path = "../../../../rust-runtime/crates/tsonic_rust_runtime" }|{ path = "../../../../target-rust/runtimes/crates/tsonic_rust_runtime" }|' \
  "$packaged_dir/Cargo.toml"

if grep -E '\.\./\.\./\.\./\.\./(rust-js|rust-runtime)' "$packaged_dir/Cargo.toml" >/dev/null; then
  echo "error: sibling-repo path dependency left in packaged Cargo.toml" >&2
  exit 1
fi

# The installed crate can land underneath an arbitrary consumer cargo
# workspace (node_modules is not workspace-aware): opt out explicitly.
if ! grep -q '^\[workspace\]$' "$packaged_dir/Cargo.toml"; then
  printf '\n[workspace]\n' >> "$packaged_dir/Cargo.toml"
fi
