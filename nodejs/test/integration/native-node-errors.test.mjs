import assert from "node:assert/strict";
import test from "node:test";
import { analyzeRust, compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { validateGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeNodeErrorSource } from "../../../../tsonic/test/fixtures/native-node-errors.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";
import { Node_Expression } from "@tsonic/target-api/source";
import { rustTargetOperationFactKey } from "../../../../tsonic-rust/dist/analysis/facts/keys.js";
import { planThrowStatement } from "../../../../tsonic-rust/dist/backend/planner/statements/errors.js";

test("Node callbacks retain native errors and exact optional error storage", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "bin" } },
    files: { "index.ts": `${nativeNodeErrorSource}\nexport function main(): void { run(); }` },
  });
  assert.deepEqual(result.diagnostics, []);
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
  assert.equal(fact.error.kind, "runtime");
  assert.equal(fact.error.boundary, "provider-native");
  assert.equal(fact.error.expression, Node_Expression(ast, statement));
  assert.deepEqual(fact.error.carrier, { kind: "target-specific", target: "rust", name: "named-type", value: {
    id: "rust.node.NodeError", path: "tsonic_rust_node::NodeError",
    traits: { implementations: [{ traitPath: "core::clone::Clone", requirements: [] }] },
    genericArguments: [], genericDefaults: [], upcasts: [],
  } });
  assert.deepEqual(program.providerErrorCarriers, [fact.error.carrier]);
  assert.ok(Object.isFrozen(fact.error));
  for (const mutation of [
    { error: { ...fact.error, expression: declaration } },
    { error: { ...fact.error, carrier: { kind: "target-named", id: "example.UnregisteredError" } } },
    { error: { ...fact.error, boundary: "target-runtime" } },
    { error: fact.error, providerErrorCarriers: [] },
  ]) {
    const diagnostics = [];
    const facts = { ...program.facts, getFact: (subject, key) => subject === statement && key === rustTargetOperationFactKey
      ? { ...fact, error: mutation.error } : program.facts.getFact(subject, key) };
    const changed = { ...program, facts, providerErrorCarriers: mutation.providerErrorCarriers ?? program.providerErrorCarriers };
    assert.equal(planThrowStatement(statement, { input: { program: changed }, sourceFile: ast.getSourceFile(statement), diagnostics,
      fallibleBoundary: { componentId: "source", errorDomain: "runtime", errorTypePath: "rt::TsonicError" },
    }), undefined);
    assert.equal(diagnostics.length, 1);
    assert.match(diagnostics[0].message, /exact source operand or native Error carrier/u);
  }
});

test("native callback errors do not promise mutable source Error storage", () => {
  for (const source of [
    `import type { NodeError } from "node:util";
     export function change(error: NodeError): void { error.message = "changed"; }`,
    `import type { Readable } from "node:stream";
     export function observe(source: Readable): void {
       source.on("error", (error: Error) => { console.log(error.stack); });
     }`,
  ]) {
    assert.throws(() => compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
      files: { "index.ts": source } }), /TypeScript diagnostics:/u);
  }
});
