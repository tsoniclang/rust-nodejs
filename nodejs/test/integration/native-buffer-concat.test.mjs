import { assertNoTargetDiagnostics } from "../../../../tsonic/test/scripts/diagnostic-assertions.mjs";
import assert from "node:assert/strict";
import test from "node:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, validateGeneratedProject, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeOwnershipCostSupport } from "../../../../tsonic-rust/test/helpers/native-ownership-cost.mjs";
import { nativeBufferConcatSource } from "../../../../tsonic/test/fixtures/native-buffer-concat.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

test("native Buffer concat consumes dense input and preserves bounded output", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "bin" } }, files: {
      "index.ts": `${nativeBufferConcatSource}\nexport function main(): void {
        if (!run()) throw new Error("concat behavior");
      }`,
    } });
  assertNoTargetDiagnostics(result.diagnostics);
  validateGeneratedProject("native-buffer-concat", result.artifacts, { run: true });
});

test("native Buffer concat does not allocate an input copy", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "lib", crateName: "native_buffer_concat_cost" } },
    files: { "index.ts": nativeBufferConcatSource } });
  assertNoTargetDiagnostics(result.diagnostics);
  const root = writeGeneratedProject("native-buffer-concat-cost", result.artifacts);
  mkdirSync(join(root, "tests"), { recursive: true });
  writeFileSync(join(root, "tests/cost.rs"), `${nativeOwnershipCostSupport}
use native_buffer_concat_cost::index;
use tsonic_rust_js::array::JsArray;
use tsonic_rust_node::buffer::Buffer;

fn handwritten(buffers: JsArray<Buffer>) -> tsonic_rust_node::NodeResult<Buffer> {
    Buffer::concat(&buffers)
}

#[test]
fn concat_matches_native_output_allocation() {
    let buffers = JsArray::from_dense(vec![
        Buffer::from_string("ab", None).unwrap(),
        Buffer::from_string("cd", None).unwrap(),
    ]);
    for _ in 0..10_000 {
        let (generated, generated_cost) = measure(|| index::concatenate(buffers.clone()).unwrap());
        let (native, native_cost) = measure(|| handwritten(buffers.clone()).unwrap());
        assert_eq!(generated.len(), 4);
        assert_eq!(native.len(), 4);
        assert_eq!(generated_cost, native_cost);
        assert_eq!(generated_cost.reallocations, 0);
    }
}
`);
  runCargo(root, ["generate-lockfile", "--offline"]);
  runCargo(root, ["fmt", "--all"]);
  runCargo(root, ["fmt", "--all", "--check"]);
  runCargo(root, ["check", "--all-targets", "--locked", "--offline"]);
  runCargo(root, ["clippy", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"]);
  runCargo(root, ["test", "--release", "--locked", "--offline"]);
});

test("native Buffer concat rejects unrelated element storage", () => {
  assert.throws(() => compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    files: { "index.ts": `import { Buffer } from "node:buffer";
      export function invalid(values: string[]): Buffer { return Buffer.concat(values); }` },
  }), /TypeScript diagnostics:[\s\S]*string\[\]/u);
});
