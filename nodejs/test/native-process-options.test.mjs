import assert from "node:assert/strict";
import test from "node:test";
import { artifactText, compileRust } from "../../../tsonic-rust/test/helpers/rust-session.mjs";
import { validateGeneratedProject } from "../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { invalidNativeProcessOptionValues, nativeProcessOptionSource, nativeOptionContracts, nativeOptionSource } from "../../../tsonic/test/fixtures/native-process-options.mjs";
import { createTsonicPlugin } from "../../dist/index.js";

test("process options retain native integer storage and wide timeouts", { timeout: 300_000 }, () => {
  const { result } = compileRust({
    surfaces: ["js"], capabilities: [createTsonicPlugin()],
    files: { "index.ts": `
      import { spawnSync, SpawnSyncOptionsWithBufferEncoding } from "node:child_process";
      export function options(): SpawnSyncOptionsWithBufferEncoding {
        return { maxBuffer: 2147483647, uid: 4294967295, gid: 4294967295, timeout: 9007199254740993 };
      }
      export function run(): void { spawnSync("must-not-be-started", [], options()); }
    ` },
  });
  assert.deepEqual(result.diagnostics, []);
  assert.doesNotMatch(artifactText(result, "src/index.rs"), /u64_to_f64|usize_to_f64/u);
  validateGeneratedProject("native-process-options", result.artifacts);
});

for (const field of ["uid", "gid", "maxBuffer", "timeout"]) {
  for (const expression of [...invalidNativeProcessOptionValues, "-1", ...(field === "uid" || field === "gid" ? ["4294967296"] : [])]) {
    test(`process ${field} rejects unrepresentable native input ${expression}`, () => {
      const { result } = compileRust({
        surfaces: ["js"], capabilities: [createTsonicPlugin()],
        files: { "index.ts": nativeProcessOptionSource(field, expression) },
      });
      assert.ok(result.diagnostics.some(diagnostic => diagnostic.category === "error"));
      assert.equal(result.artifacts.length, 0);
    });
  }
}

for (const [moduleSpecifier, typeName, field] of nativeOptionContracts) {
  test(`${typeName}.${field} retains native option storage and rejects nonintegral values`, () => {
    for (const expression of ["1", ...invalidNativeProcessOptionValues]) {
      const { result } = compileRust({
        surfaces: ["js"], capabilities: [createTsonicPlugin()],
        files: { "index.ts": nativeOptionSource(moduleSpecifier, typeName, field, expression) },
      });
      if (expression === "1") assert.deepEqual(result.diagnostics, []);
      else {
        assert.ok(result.diagnostics.some(diagnostic => diagnostic.category === "error"), expression);
        assert.equal(result.artifacts.length, 0);
      }
    }
  });
}
