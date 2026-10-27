import { assertNoTargetDiagnostics } from "../../../../tsonic/test/scripts/diagnostic-assertions.mjs";
import assert from "node:assert/strict";
import test from "node:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { analyzeRust, compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, validateGeneratedProject, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeOwnershipCostSupport } from "../../../../tsonic-rust/test/helpers/native-ownership-cost.mjs";
import { createRustRecordFixture } from "../../../../tsonic-rust/test/helpers/records.mjs";
import { nativeNodeErrorSource, nativeNodeErrorUnionSource, nativeNodeErrorUnionProofSource } from "../../../../tsonic/test/fixtures/native-node-errors.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";
import { createTargetSourceProgram, Node_Expression } from "@tsonic/target-api/source";
import { planThrowStatement } from "../../../../tsonic-rust/dist/backend/planner/statements/statement-try.js";
import { admitRustThrowCompletion } from "../../../../tsonic-rust/dist/analysis/errors/throw-admission.js";
import { admitRustConversionEmission } from "../../../../tsonic-rust/dist/analysis/conversions/emission.js";
import { recordThrowFacts } from "../../../../tsonic-rust/dist/analysis/errors/thrown-values.js";
import { createRustConversionEmissionProducer } from "../../../../tsonic-rust/dist/analysis/program/records/operations/conversion-emission.js";
import { createRustProgramRecordBuilder } from "../../../../tsonic-rust/dist/analysis/program/records/create.js";
import { createRustSourceTypeRegistry } from "../../../../tsonic-rust/dist/analysis/project-types/source-type-registry.js";
import { rustSourceFilePlanContext } from "../../../../tsonic-rust/test/helpers/plan-context.mjs";
import { planRustErrorRegion } from "../../../../tsonic-rust/dist/backend/planner/program/error-regions.js";
import { rustRetainedErrorTargetType } from "@tsonic/target-rust/provider";

test("Node callbacks retain native errors and exact optional error storage", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "bin" } },
    files: { "index.ts": (`${nativeNodeErrorSource}\nexport function main(): void { run(); }` + "\nmain();\n") },
  });
  assertNoTargetDiagnostics(result.diagnostics);
  const execution = validateGeneratedProject("native-node-errors", result.artifacts, { run: true });
  assert.match(execution.stdout, /native error/u);
  assert.match(execution.stdout, /native success/u);
  assert.match(execution.stdout, /native throw/u);
});

test("native callback throwing requires exact operand, carrier and registration", () => {
  const { source, program } = analyzeRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()], files: {
    "index.ts": `import type { NodeError } from "node:util";
      export function fail(error: NodeError): never { throw error; }`,
  } });
  const checkedSource = createTargetSourceProgram(source);
  const { ast } = program.source;
  const declaration = program.source.sourceFiles.flatMap(file => ast.statements(file))
    .find(node => ast.text(ast.name(node)) === "fail");
  assert.ok(declaration);
  const statement = ast.statements(ast.body(declaration))[0];
  const fact = program.records.targetOperation.get(statement);
  assert.equal(fact.kind, "throw-op");
  assert.equal(fact.error.kind, "conversion");
  assert.deepEqual(fact.error.conversion.route, { kind: "runtime", boundary: "provider-native" });
  assert.equal(fact.error.expression === Node_Expression(ast, statement), true, "exact checked throw operand");
  assert.deepEqual(fact.error.conversion.source, { kind: "target-specific", target: "rust", name: "named-type", value: {
    id: "rust.node.NodeError", path: "tsonic_rust_node::NodeError",
    traits: { implementations: [
      { trait: { kind: "trait-ref", id: "core::clone::Clone", path: "core::clone::Clone", genericArguments: [], associatedConstraints: [] }, requirements: [] },
      { trait: { kind: "trait-ref", id: "core::convert::Into", path: "core::convert::Into", genericArguments: [{ kind: "type", type: rustRetainedErrorTargetType() }], associatedConstraints: [] }, requirements: [] },
      ...["tsonic_rust_runtime::ErrorObject", "tsonic_rust_runtime::ErrorStack", "tsonic_rust_runtime::ToSourceString"]
        .map(path => ({ trait: { kind: "trait-ref", id: path, path, genericArguments: [], associatedConstraints: [] }, requirements: [] })),
    ] },
    genericArguments: [], genericDefaults: [], upcasts: [],
  } });
  assert.deepEqual(program.providerErrorCarriers, [fact.error.conversion.source]);
  assert.ok(Object.isFrozen(fact.error));
  const completion = program.records.throwCompletion.get(statement);
  const region = program.errorRegions.get(declaration);
  assert.equal(completion?.operation === fact, true, "the admitted throw retains its original selected operation");
  const selected = rustSourceFilePlanContext(program, statement);
  assert.equal(selected.sourcePackageErrors !== undefined, true, "checked source-package errors are admitted");
  const renderedRegion = planRustErrorRegion(declaration, selected);
  assert.equal(renderedRegion !== undefined, true, "the throw has its actual checked package error owner");
  const rendered = planThrowStatement(statement, { ...selected, callableDeclaration: declaration,
    fallibleBoundary: renderedRegion.boundary });
  assert.equal(rendered.length, 1, "one admitted throw produces one native throw");
  assert.equal(rendered[0].kind, "throw");
  assertNoTargetDiagnostics(selected.diagnostics);
  for (const [label, mutation, message] of [
    ["wrong source operand", { error: { ...fact.error, expression: declaration } }, /exact selected source operation/u],
    ["wrong native carrier", { error: { ...fact.error, conversion: { ...fact.error.conversion, source: { kind: "target-named", id: "example.UnregisteredError" } } } }, /exact selected native operand carrier/u],
    ["wrong native error domain", { error: { ...fact.error, conversion: { ...fact.error.conversion, route: { kind: "runtime", boundary: "target-runtime" } } } }, undefined],
    ["unregistered native error", { error: fact.error, providerErrorCarriers: [] }, undefined],
  ]) {
    const records = createRustRecordFixture({
      targetOperation: { get: subject => subject === statement
        ? { ...fact, error: mutation.error } : program.records.targetOperation.get(subject) },
    }, program.records);
    if (message !== undefined) {
      let admitted;
      let failure;
      try {
        admitted = admitRustThrowCompletion({ ast, records, region,
          componentId: selected.sourcePackageComponentId }, statement);
      } catch (error) {
        failure = error;
      }
      assert.equal(failure?.diagnostic?.code, "TSONIC_INTERNAL_ANALYSIS_INVARIANT", label);
      assert.equal(message.test(failure?.diagnostic?.message ?? ""), true, label);
      assert.equal(admitted === undefined, true, label + ": malformed evidence is never published");
    } else {
      if (mutation.providerErrorCarriers === undefined) {
        const conversions = createRustConversionEmissionProducer(() => {});
        let admitted;
        assert.throws(() => {
          admitRustConversionEmission(conversions, mutation.error.conversion, program.typeDefinitions);
          admitted = admitRustThrowCompletion({ ast, records, region,
            componentId: selected.sourcePackageComponentId }, statement);
        }, failure => failure?.diagnostic?.code === "TSONIC_INTERNAL_ANALYSIS_INVARIANT" &&
          failure.diagnostic.message === "An admitted native conversion requires its exact physical conversion contract.", label);
        assert.equal(conversions.records.conversionContract.get(mutation.error.conversion) === undefined, true,
          label + ": the malformed conversion has no published emission contract");
        assert.equal(admitted === undefined, true, label + ": the malformed conversion cannot reach throw completion");
        const original = admitRustConversionEmission(conversions, fact.error.conversion, program.typeDefinitions);
        assert.equal(conversions.records.conversionContract.get(fact.error.conversion) === original, true,
          label + ": the same admission owner publishes the original native conversion");
      } else {
        const inherited = createRustRecordFixture({
          targetOperation: { get: subject => subject === statement ? undefined : program.records.targetOperation.get(subject) },
          throwCompletion: { get: subject => subject === statement ? undefined : program.records.throwCompletion.get(subject) },
        }, program.records);
        const createWalk = providerErrorCarriers => ({
          context: { ast, source: checkedSource, typeDefinitions: program.typeDefinitions,
            records: createRustProgramRecordBuilder(program.typeDefinitions, inherited),
            diagnostics: [], requiredRecords: [], semanticsFor: node => checkedSource.semantics.forNode(node) },
          providerErrorCarriers, sourceTypes: createRustSourceTypeRegistry(undefined, program.typeDefinitions),
          resolving: new Set(), rejectedExpressions: new WeakSet(),
        });
        const registered = createWalk(program.providerErrorCarriers);
        recordThrowFacts(registered, statement, ast.getSourceFile(statement));
        assertNoTargetDiagnostics(registered.context.diagnostics);
        assert.equal(registered.context.records.targetOperation.get(statement)?.kind, "throw-op",
          label + ": the same recording boundary publishes the registered native throw");
        const original = admitRustThrowCompletion({ ast, records: registered.context.records, region,
          componentId: selected.sourcePackageComponentId }, statement);
        assert.equal(original.kind, "resolved", label + ": the registered throw reaches completion admission");
        const unregistered = createWalk(mutation.providerErrorCarriers);
        recordThrowFacts(unregistered, statement, ast.getSourceFile(statement));
        assert.equal(unregistered.context.diagnostics.length, 1, label);
        const diagnostic = unregistered.context.diagnostics[0];
        assert.equal(diagnostic?.code, "RUST_MISSING_TARGET_FACT", label);
        assert.equal(diagnostic?.category, "error", label);
        assert.equal(diagnostic?.source, "tsonic-rust", label);
        assert.equal(diagnostic?.message, "Runtime error construction has no exact registered native error carrier.", label);
        assert.equal(diagnostic?.sourceNode === statement, true, label + ": refusal belongs to the original throw");
        assert.deepEqual(diagnostic?.evidence, ["target.capability=rust.backend.throw-runtime-error-route"], label);
        assert.equal(unregistered.rejectedExpressions.has(statement), true, label + ": the throw is recorded as rejected");
        assert.equal(unregistered.context.records.targetOperation.get(statement) === undefined, true,
          label + ": the unregistered native throw has no published operation");
        assert.equal(unregistered.context.records.throwCompletion.get(statement) === undefined, true,
          label + ": the unregistered native throw has no published completion");
        assert.equal(unregistered.context.requiredRecords.length, 0, label + ": no rejected operation enters the required-record domain");
      }
    }
    assert.equal(program.records.targetOperation.get(statement) === fact, true, label + ": the original operation is unchanged");
    assert.equal(program.records.throwCompletion.get(statement) === completion, true, label + ": the original completion is unchanged");
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
  writeFileSync(join(root, "tests/ownership.rs"), `${nativeOwnershipCostSupport(root)}
use native_node_error_cost::{index, program};
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
            Err(program::ErrorTransport::Runtime(rt::TsonicError::Node { code, source: original })) => {
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
    files: { "index.ts": (`${nativeNodeErrorUnionProofSource}\nexport function main(): void {
      runUnions();
    }` + "\nmain();\n") },
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
  writeFileSync(join(root, "tests/ownership.rs"), `${nativeOwnershipCostSupport(root)}
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
