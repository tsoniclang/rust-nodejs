import assert from "node:assert/strict";
import test from "node:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { compileRust, artifactText } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeOwnershipCostSupport } from "../../../../tsonic-rust/test/helpers/native-ownership-cost.mjs";
import { nativeFileOffsetDeclarations, nativeFileOffsetsSource } from "../../../../tsonic/test/fixtures/native-file-offsets.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

test("selected native file offsets retain width through nullish utilities and aliases", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "lib", crateName: "native_file_offsets" } },
    files: { "index.ts": nativeFileOffsetsSource } });
  assert.deepEqual(result.diagnostics, []);
  const output = artifactText(result, "src/index.rs");
  assert.match(output, /fn exact\(value: u64\) -> u64/u);
  assert.match(output, /fn alias\(value: u64\) -> u64/u);
  assert.match(output, /fn present\(value: Option<u64>\) -> u64/u);
  assert.doesNotMatch(output, /f64|f32|Option<Option/u);
  const root = writeGeneratedProject("native-file-offsets", result.artifacts);
  mkdirSync(join(root, "tests"), { recursive: true });
  writeFileSync(join(root, "tests/offsets.rs"), `${nativeOwnershipCostSupport}
use native_file_offsets::index;

#[test]
fn native_offset_precision_and_cost() {
    for value in [0, 9_007_199_254_740_993, u64::MAX] {
        assert_eq!(index::exact(value), value);
        assert_eq!(index::alias(value), value);
        assert_eq!(index::present(Some(value)), value);
        assert_eq!(index::decimalQuotient(value), value / 10);
    }
    assert_eq!(index::present(None), 0);
    for _ in 0..10_000 {
        let (result, cost) = measure(|| (index::exact(u64::MAX), index::alias(u64::MAX), index::decimalQuotient(u64::MAX)));
        assert_eq!(result, (u64::MAX, u64::MAX, u64::MAX / 10));
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        assert_eq!(cost.allocated_bytes, 0);
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

test("non-nullish native offsets reject an absent source return", () => {
  assert.throws(() => compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    files: { "index.ts": `${nativeFileOffsetDeclarations} export function invalid(value: Offset | null): Offset { return value; }` },
  }), /TypeScript diagnostics:/u);
});
