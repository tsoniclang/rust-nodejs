import assert from "node:assert/strict";
import test from "node:test";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { validateGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";
import { nativeRetainedErrorCases, nativeRetainedErrorSourceFor, nativeRetainedTerminalErrorSource } from "../../../../tsonic/test/fixtures/native-retained-errors.mjs";

for (const [parameter, storage, ordering] of nativeRetainedErrorCases) {
    test(`retaining native streams preserve original Error subtype, identity and live fields (${parameter}, ${storage}, ${ordering})`, { timeout: 300_000 }, () => {
      const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
        target: { id: "rust", options: { outputType: "bin" } }, files: {
          "index.ts": `${nativeRetainedErrorSourceFor(parameter, storage, ordering)}\nexport function main(): void { run(); }`,
        } });
      assert.equal(result.diagnostics.length, 0, result.diagnostics.map(row => row.message).join("\n"));
      const execution = validateGeneratedProject(`native-retained-errors-${parameter}-${storage}-${ordering}`, result.artifacts, { run: true });
      assert.equal(execution.status, 0);
    });
}

test("terminal source failures preserve identity, native cleanup and subsequent close delivery", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "bin" } }, files: {
      "index.ts": `${nativeRetainedTerminalErrorSource}\nexport function main(): void { if (!run()) throw new Error("terminal source failure lost"); }`,
    } });
  assert.equal(result.diagnostics.length, 0, result.diagnostics.map(row => row.message).join("\n"));
  const execution = validateGeneratedProject("native-retained-terminal-errors", result.artifacts, { run: true });
  assert.equal(execution.status, 0);
});
