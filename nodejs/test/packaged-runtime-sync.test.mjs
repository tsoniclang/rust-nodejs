import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

test("the npm artifact declares one canonical Node runtime tree", () => {
  const manifest = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8"));
  assert.ok(manifest.files.includes("rust/crates"));
  assert.ok(manifest.files.includes("!rust/crates/**/.temp/**"));
  assert.ok(manifest.files.includes("!dist/**/*.tsbuildinfo"));
  assert.ok(manifest.files.includes("rust/tests"));
  assert.ok(!manifest.files.includes("rust"));
  assert.ok(!manifest.files.includes("Cargo.toml"));
  assert.ok(!manifest.files.includes("Cargo.lock"));
  assert.ok(!manifest.files.includes("runtimes"));
  assert.ok(!existsSync(join(repoRoot, "runtimes/crates/tsonic_rust_node/src/lib.rs")), "duplicate runtime source tree must not exist");
  assert.ok(existsSync(join(repoRoot, "rust/crates/tsonic_rust_node/src/lib.rs")));
});

test("canonical Node crate dependencies are independent of npm filesystem layout", () => {
  const cargo = readFileSync(join(repoRoot, "rust/crates/tsonic_rust_node/Cargo.toml"), "utf8");
  assert.match(cargo, /tsonic_rust_js = "=0\.1\.0"/u);
  assert.match(cargo, /tsonic_rust_runtime = "=0\.1\.0"/u);
  assert.doesNotMatch(cargo, /tsonic_rust_(?:js|runtime)\s*=\s*\{\s*path\s*=/u);
});
