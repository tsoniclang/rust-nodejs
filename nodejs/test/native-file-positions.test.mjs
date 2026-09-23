import assert from "node:assert/strict";
import test from "node:test";
import { artifactText, compileRust } from "../../../tsonic-rust/test/helpers/rust-session.mjs";
import { validateGeneratedProject } from "../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeFilePositionSource } from "../../../tsonic/test/fixtures/native-file-positions.mjs";
import { createTsonicPlugin } from "../../dist/index.js";

test("filesystem calls preserve exact native int64 positions through provider selection", { timeout: 300_000 }, () => {
  const { result } = compileRust({
    surfaces: ["js"], capabilities: [createTsonicPlugin()], files: { "index.ts": nativeFilePositionSource },
  });
  assert.deepEqual(result.diagnostics, []);
  const output = artifactText(result, "src/index.rs");
  assert.match(output, /position: i64/u);
  assert.match(output, /9007199254740993/u);
  assert.match(output, /read_sync_buffer_number/u);
  assert.match(output, /write_sync_buffer_number/u);
  assert.doesNotMatch(output, /position as (?:f64|i32)/u);
  validateGeneratedProject("native-file-positions", result.artifacts);
});
