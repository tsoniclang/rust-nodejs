import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

test("committed runtimes crate is identical to rust/crates source", () => {
  try {
    execFileSync(
      "diff",
      ["-r", "--exclude=target", "rust/crates/tsonic_rust_node", "runtimes/crates/tsonic_rust_node"],
      { cwd: repoRoot, encoding: "utf8" },
    );
  } catch (error) {
    assert.fail(`runtimes copy is stale; run scripts/package-runtime.sh\n${error.stdout ?? error.message}`);
  }
});
