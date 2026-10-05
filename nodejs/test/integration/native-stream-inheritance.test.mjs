import assert from "node:assert/strict";
import test from "node:test";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { validateGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeStreamInheritanceSource, invalidNativeStreamInheritanceSources } from "../../../../tsonic/test/fixtures/native-stream-inheritance.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

test("inherited file stream methods and erased codec destruction use the exact native base API", { timeout: 300_000 }, () => {
  const { result } = compileRust({
    surfaces: ["js"],
    capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "bin" } },
    files: { "index.ts": `${nativeStreamInheritanceSource}\nexport function main(): void { run(); }` },
  });
  assert.equal(result.diagnostics.length, 0, result.diagnostics.map(row => row.message).join("\n"));
  const execution = validateGeneratedProject("native-stream-inheritance", result.artifacts, { run: true });
  assert.equal(execution.status, 0);
});

for (const [name, source] of invalidNativeStreamInheritanceSources) {
  test(`inherited native stream declarations reject ${name} before publication`, () => {
    assert.throws(() => compileRust({
      surfaces: ["js"],
      capabilities: [createTsonicPlugin()],
      files: { "index.ts": source },
    }), /TypeScript diagnostics:/);
  });
}
