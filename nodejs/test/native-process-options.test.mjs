import assert from "node:assert/strict";
import test from "node:test";
import { acmeTestingPackage, artifactText, compileRust } from "../../../tsonic-rust/test/helpers/rust-session.mjs";
import { validateGeneratedProject } from "../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { invalidNativeProcessOptionValues, nativeProcessOptionSource, nativeOptionContracts, nativeOptionSource, nativeOptionRuntimeSource } from "../../../tsonic/test/fixtures/native-process-options.mjs";
import { createTsonicPlugin } from "../../dist/index.js";

test("Buffer byte construction retains native array elements without a floating or copied input", { timeout: 300_000 }, () => {
  const { result } = compileRust({
    surfaces: ["js"], capabilities: [createTsonicPlugin()], packages: [acmeTestingPackage()],
    target: { id: "rust", options: { outputType: "bin", crateName: "native_buffer_elements" } },
    files: { "index.ts": `
      import { check } from "@acme/testing";
      import { Buffer } from "node:buffer";
      import type { nativeUint } from "@tsonic/core/types.js";
      export function main(): void {
        const exact: nativeUint = 9007199254740993;
        const input = [exact];
        const output = Buffer.from(input);
        const replacement: nativeUint = 7;
        input[0] = replacement;
        check(output.readUInt8() === 1);
        check(input[0] === 7);
        const floating = Buffer.from([65.9, -1.9, Number.NaN]);
        check(floating.readUInt8() === 65 && floating.readUInt8(1) === 255 && floating.readUInt8(2) === 0);
      }
    ` },
  });
  assert.deepEqual(result.diagnostics, []);
  assert.doesNotMatch(artifactText(result, "src/index.rs"), /usize_to_f64|to_vec\(\)|\.values\(\)/u);
  validateGeneratedProject("native-buffer-elements", result.artifacts, { run: true });
});

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
  for (const expression of ["0.5", "-1", ...(field === "uid" || field === "gid" ? ["4294967296"] : [])]) {
    test(`process ${field} rejects unrepresentable native input ${expression}`, () => {
      const { result } = compileRust({
        surfaces: ["js"], capabilities: [createTsonicPlugin()],
        files: { "index.ts": nativeProcessOptionSource(field, expression, { inline: true }) },
      });
      assert.ok(result.diagnostics.some(diagnostic => diagnostic.category === "error"));
      assert.equal(result.artifacts.length, 0);
    });
  }
}

for (const [moduleSpecifier, typeName, field] of nativeOptionContracts) {
  test(`${typeName}.${field} selects exact native option storage for numeric variables`, () => {
    for (const expression of ["1", ...invalidNativeProcessOptionValues]) {
      const { result } = compileRust({
        surfaces: ["js"], capabilities: [createTsonicPlugin()],
        files: { "index.ts": nativeOptionSource(moduleSpecifier, typeName, field, expression) },
      });
      assert.deepEqual(result.diagnostics, []);
      assert.match(artifactText(result, "src/index.rs"), /checked_integer::</u);
    }
  });
}

test("native option fields reject invalid dynamic numbers before use", { timeout: 300_000 }, () => {
  const { result } = compileRust({
    surfaces: ["js"], capabilities: [createTsonicPlugin()], packages: [acmeTestingPackage()],
    target: { id: "rust", options: { outputType: "bin", crateName: "native_option_fields" } },
    files: { "index.ts": `
      import { check } from "@acme/testing";
      import type { nativeUint } from "@tsonic/core/types.js";
      import type { RmOptions } from "node:fs";
      ${nativeOptionRuntimeSource}
      function wide(value: nativeUint): RmOptions { return { retryDelay: value }; }
      export function main(): void {
        check(run());
        const value: nativeUint = 9007199254740993;
        const options = wide(value);
        if (options.retryDelay === undefined) throw new Error("missing native option");
        check(options.retryDelay === 9007199254740993);
      }
    ` },
  });
  assert.deepEqual(result.diagnostics, []);
  assert.doesNotMatch(artifactText(result, "src/index.rs"), /u64_to_f64|usize_to_f64/u);
  validateGeneratedProject("native-option-fields", result.artifacts, { run: true });
});
