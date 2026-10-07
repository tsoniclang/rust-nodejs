import { assertNoTargetDiagnostics } from "../../../../tsonic/test/scripts/diagnostic-assertions.mjs";
import assert from "node:assert/strict";
import test from "node:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { analyzeRust, compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, validateGeneratedProject, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeOwnershipCostSupport } from "../../../../tsonic-rust/test/helpers/native-ownership-cost.mjs";
import { nativeNodeErrorSource, nativeNodeErrorUnionSource, nativeNodeErrorUnionProofSource } from "../../../../tsonic/test/fixtures/native-node-errors.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";
import { Node_Expression } from "@tsonic/target-api/source";
import { rustTargetOperationFactKey } from "../../../../tsonic-rust/dist/analysis/facts/keys.js";
import { planThrowStatement } from "../../../../tsonic-rust/dist/backend/planner/statements/errors.js";

test("Node callbacks retain native errors and exact optional error storage", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "bin" } },
    files: { "index.ts": `${nativeNodeErrorSource}\nexport function main(): void { run(); }` },
  });
  assertNoTargetDiagnostics(result.diagnostics);
  const execution = validateGeneratedProject("native-node-errors", result.artifacts, { run: true });
  assert.match(execution.stdout, /native error/u);
  assert.match(execution.stdout, /native success/u);
  assert.match(execution.stdout, /native throw/u);
});

test("native callback throwing requires exact operand, carrier and registration", () => {
  const { program } = analyzeRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()], files: {
    "index.ts": `import type { NodeError } from "node:util";
      export function fail(error: NodeError): never { throw error; }`,
  } });
  const { ast } = program.source;
  const declaration = program.source.sourceFiles.flatMap(file => ast.statements(file))
    .find(node => ast.text(ast.name(node)) === "fail");
  assert.ok(declaration);
  const statement = ast.statements(ast.body(declaration))[0];
  const fact = program.facts.getFact(statement, rustTargetOperationFactKey);
  assert.equal(fact.kind, "throw-op");
  assert.equal(fact.error.kind, "conversion");
  assert.deepEqual(fact.error.conversion.route, { kind: "runtime", boundary: "provider-native" });
  assert.equal(fact.error.expression, Node_Expression(ast, statement));
  assert.deepEqual(fact.error.conversion.source, { kind: "target-specific", target: "rust", name: "named-type", value: {
    id: "rust.node.NodeError", path: "tsonic_rust_node::NodeError",
    traits: { implementations: ["core::clone::Clone", "tsonic_rust_runtime::ErrorObject",
      "tsonic_rust_runtime::ErrorStack", "tsonic_rust_runtime::ToSourceString"].map(traitPath => ({ traitPath, requirements: [] })) },
    genericArguments: [], genericDefaults: [], upcasts: [],
  } });
  assert.deepEqual(program.providerErrorCarriers, [fact.error.conversion.source]);
  assert.ok(Object.isFrozen(fact.error));
  for (const [mutation, message] of [
    [{ error: { ...fact.error, expression: declaration } }, "Finalized throw fact conflicts with its exact source operand or native Error carrier."],
    [{ error: { ...fact.error, conversion: { ...fact.error.conversion, source: { kind: "target-named", id: "example.UnregisteredError" } } } }, "Finalized throw fact conflicts with its exact source operand or native Error carrier."],
    [{ error: { ...fact.error, conversion: { ...fact.error.conversion, route: { kind: "runtime", boundary: "target-runtime" } } } }, "Program error construction requires an exact carrier and error domain."],
    [{ error: fact.error, providerErrorCarriers: [] }, "Runtime error construction has no exact registered native error carrier."],
  ]) {
    const diagnostics = [];
    const facts = { ...program.facts, getFact: (subject, key) => subject === statement && key === rustTargetOperationFactKey
      ? { ...fact, error: mutation.error } : program.facts.getFact(subject, key) };
    const changed = { ...program, facts, providerErrorCarriers: mutation.providerErrorCarriers ?? program.providerErrorCarriers };
    assert.equal(planThrowStatement(statement, { input: { program: changed }, sourceFile: ast.getSourceFile(statement), diagnostics,
      fallibleBoundary: { componentId: "source", errorDomain: "runtime", errorTypePath: "rt::TsonicError" },
    }), undefined);
    assert.equal(diagnostics.length, 1);
    assert.equal(diagnostics[0].message, `${message} Node kind: KindIdentifier.`);
  }
});

test("native callback errors do not promise mutable source Error storage", () => {
  for (const source of [
    `import type { NodeError } from "node:util";
     export function change(error: NodeError): void { error.message = "changed"; }`,
    `import type { NodeError } from "node:util";
     export function change(error: NodeError): void { error.name = "changed"; }`,
    `import type { NodeError } from "node:util";
     export function change(error: NodeError): void { error.stack = "changed"; }`,
  ]) {
    assert.throws(() => compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
      files: { "index.ts": source } }), /TypeScript diagnostics:/u);
  }
});

test("last-use native errors move without allocation and retain later control-flow uses", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "lib", crateName: "native_node_error_cost" } },
    files: { "index.ts": nativeNodeErrorSource },
  });
  assertNoTargetDiagnostics(result.diagnostics);
  const root = writeGeneratedProject("native-node-error-cost", result.artifacts);
  mkdirSync(join(root, "tests"), { recursive: true });
  writeFileSync(join(root, "tests/ownership.rs"), `${nativeOwnershipCostSupport}
use native_node_error_cost::index;
use tsonic_rust_node::NodeError;
use tsonic_rust_runtime as rt;

fn handwritten(error: Option<NodeError>) -> Result<(), rt::TsonicError> {
    let Some(error) = error else { return Ok(()); };
    if error.message().is_empty() {
        return Err(rt::JsError::error("empty native error").into());
    }
    Err(error.into())
}

#[test]
fn preserves_owned_payload_and_matches_native_cost() {
    for _ in 0..10_000 {
        let error = NodeError::new("ERR_NATIVE_ORIGINAL", "original message");
        let code_pointer = error.code.as_ptr();
        let source = error.source_error().clone();
        let (result, generated_cost) = measure(|| index::consume(Some(error)));
        match result {
            Err(rt::TsonicError::Node { code, source: original }) => {
                assert_eq!(code.as_ptr(), code_pointer);
                assert_eq!(code, "ERR_NATIVE_ORIGINAL");
                assert_eq!(original, source);
            }
            other => panic!("native payload was changed: {other:?}"),
        }
        let native_error = NodeError::new("ERR_NATIVE_ORIGINAL", "original message");
        let (native_result, native_cost) = measure(|| handwritten(Some(native_error)));
        assert!(native_result.is_err());
        assert_eq!(generated_cost, native_cost);
        assert_eq!(generated_cost.allocations, 0);
        assert_eq!(generated_cost.reallocations, 0);
    }
    let (absent, absent_cost) = measure(|| index::consume(None));
    assert!(absent.is_ok());
    assert_eq!(absent_cost, Cost::default());
    assert!(index::consume(Some(NodeError::new("ERR_EMPTY", ""))).is_err());
}

#[test]
fn catch_finally_and_loop_reads_keep_the_original_value_available() {
    let error = NodeError::new("ERR_RETAINED", "retained message");
    assert_eq!(index::retained(error.clone()), "retained message");
    assert_eq!(index::finalized(error.clone()), "retained message");
    assert_eq!(index::repeated(error.clone()), "retained message");
    assert!(index::captured(error).unwrap());
}
`);
  runCargo(root, ["generate-lockfile", "--offline"]);
  runCargo(root, ["fmt", "--all"]);
  runCargo(root, ["fmt", "--all", "--check"]);
  runCargo(root, ["check", "--all-targets", "--locked", "--offline"]);
  runCargo(root, ["clippy", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"]);
  runCargo(root, ["test", "--release", "--locked", "--offline"]);
});

test("closed native error alternatives preserve every original error route", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "bin" } },
    files: { "index.ts": `${nativeNodeErrorUnionProofSource}\nexport function main(): void {
      runUnions();
    }` },
  });
  assert.equal(result.diagnostics.length, 0,
    result.diagnostics.map(({ code, message }) => `${code}: ${message}`).join("\n"));
  const execution = validateGeneratedProject("native-node-error-unions", result.artifacts, { run: true });
  assert.equal(execution.stdout.match(/native union routes/gu)?.length, 1);
});

test("native error union dispatch preserves owned payloads without allocation", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "lib", crateName: "native_node_error_union_cost" } },
    files: { "index.ts": nativeNodeErrorUnionSource },
  });
  assertNoTargetDiagnostics(result.diagnostics);
  const root = writeGeneratedProject("native-node-error-union-cost", result.artifacts);
  mkdirSync(join(root, "tests"), { recursive: true });
  writeFileSync(join(root, "tests/ownership.rs"), `${nativeOwnershipCostSupport}
use native_node_error_union_cost::index;
use tsonic_rust_node::NodeError;
use tsonic_rust_runtime as rt;

#[test]
fn closed_dispatch_matches_native_transport_cost_and_identity() {
    for _ in 0..10_000 {
        let error = NodeError::new("ERR_UNION_ORIGINAL", "original union message");
        let pointer = error.code.as_ptr();
        let source = error.source_error().clone();
        let (result, generated_cost) = measure(|| index::throwNative(error));
        match result {
            Err(rt::TsonicError::Node { code, source: original }) => {
                assert_eq!(code.as_ptr(), pointer);
                assert_eq!(original, source);
            }
            other => panic!("original native error was not retained: {other:?}"),
        }
        let native = NodeError::new("ERR_UNION_ORIGINAL", "original union message");
        let (_, native_cost) = measure(|| rt::TsonicError::from(native));
        assert_eq!(generated_cost, native_cost);
        assert_eq!(generated_cost, Cost::default());
        let original = rt::JsError::error("source union message");
        let identity = original.clone();
        let (result, source_cost) = measure(|| index::throwSource(original));
        match result {
            Err(rt::TsonicError::Js(source)) => assert_eq!(source, identity),
            other => panic!("original source error was not retained: {other:?}"),
        }
        assert_eq!(source_cost, Cost::default());
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
