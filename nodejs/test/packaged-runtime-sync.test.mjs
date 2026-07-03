import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

test("committed runtimes crate is identical to rust/crates source", () => {
  try {
    execFileSync(
      "diff",
      // Cargo.toml is transformed at packaging (lib-only: [[test]] sections
      // are stripped), so it is compared by policy below, not byte equality.
      ["-r", "--exclude=target", "--exclude=.temp", "--exclude=Cargo.toml", "rust/crates/tsonic_rust_node", "runtimes/crates/tsonic_rust_node"],
      { cwd: repoRoot, encoding: "utf8" },
    );
  } catch (error) {
    assert.fail(`runtimes copy is stale; run scripts/package-runtime.sh\n${error.stdout ?? error.message}`);
  }
});

// Sibling-repo path deps in the source crate are rewritten at packaging time
// to the flat node_modules peer layout (@tsonic/target-rust ships the runtime
// crates under runtimes/crates/).
const peerLayoutRewrites = new Map([
  [
    'tsonic_rust_js = { path = "../../../../rust-js/crates/tsonic_rust_js" }',
    'tsonic_rust_js = { path = "../../../../target-rust/runtimes/crates/tsonic_rust_js" }',
  ],
  [
    'tsonic_rust_runtime = { path = "../../../../rust-runtime/crates/tsonic_rust_runtime" }',
    'tsonic_rust_runtime = { path = "../../../../target-rust/runtimes/crates/tsonic_rust_runtime" }',
  ],
]);

test("packaged manifest is lib-only with dependencies preserved", () => {
  const packaged = readFileSync(join(repoRoot, "runtimes/crates/tsonic_rust_node/Cargo.toml"), "utf8");
  const source = readFileSync(join(repoRoot, "rust/crates/tsonic_rust_node/Cargo.toml"), "utf8");
  assert.ok(!packaged.includes("[[test]]"), "packaged manifest must not declare repo-relative test targets");
  const dependencyLines = source.split("\n").filter((line) => line.includes(" = ") && !line.startsWith("name") && !line.startsWith("version") && !line.startsWith("edition") && !line.startsWith("path"));
  for (const line of dependencyLines) {
    const expected = peerLayoutRewrites.get(line) ?? line;
    assert.ok(packaged.includes(expected), `packaged manifest missing dependency line: ${expected}`);
  }
});

test("packaged manifest uses peer-layout paths for runtime crate dependencies", () => {
  const packaged = readFileSync(join(repoRoot, "runtimes/crates/tsonic_rust_node/Cargo.toml"), "utf8");
  for (const rewritten of peerLayoutRewrites.values()) {
    assert.ok(packaged.includes(rewritten), `packaged manifest missing peer-layout dependency: ${rewritten}`);
  }
  assert.ok(!packaged.includes("../../../../rust-js"), "packaged manifest must not reference the rust-js sibling repo");
  assert.ok(!packaged.includes("../../../../rust-runtime"), "packaged manifest must not reference the rust-runtime sibling repo");
});
