import assert from "node:assert/strict";
import test from "node:test";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { validateGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";
import { nativeRetainedErrorSource } from "../../../../tsonic/test/fixtures/native-retained-errors.mjs";

test("retaining native streams preserve the original Error subtype, identity and live fields", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "bin" } }, files: {
      "index.ts": `${nativeRetainedErrorSource}\nexport function main(): void { run(); }`,
    } });
  assert.deepEqual(result.diagnostics, []);
  const execution = validateGeneratedProject("native-retained-errors", result.artifacts, { run: true });
  assert.equal(execution.status, 0);
});
