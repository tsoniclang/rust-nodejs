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

test("packaged manifest is lib-only with dependencies preserved", () => {
  const packaged = readFileSync(join(repoRoot, "runtimes/crates/tsonic_rust_node/Cargo.toml"), "utf8");
  const source = readFileSync(join(repoRoot, "rust/crates/tsonic_rust_node/Cargo.toml"), "utf8");
  assert.ok(!packaged.includes("[[test]]"), "packaged manifest must not declare repo-relative test targets");
  const dependencyLines = source.split("\n").filter((line) => line.includes(" = ") && !line.startsWith("name") && !line.startsWith("version") && !line.startsWith("edition") && !line.startsWith("path"));
  for (const line of dependencyLines) {
    assert.ok(packaged.includes(line), `packaged manifest missing dependency line: ${line}`);
  }
});
