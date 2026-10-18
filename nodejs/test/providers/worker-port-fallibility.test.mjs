import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";
import { rustRetainedErrorTargetType } from "@tsonic/target-rust/provider";

function assertPortFallibility(context) {
  const [contribution] = createTsonicPlugin().createTargetContributions(context);
  const { modules, operations } = contribution.definition;
  const module = modules.find((entry) => entry.moduleSpecifier === "node:worker_threads");
  assert.ok(module);
  const port = module.exports.find((entry) => entry.name === "MessagePort");
  assert.ok(port);
  const start = port.members.find((member) => member.name === "start");
  assert.ok(start);
  assert.deepEqual(start.signatures.map((signature) => signature.returnType), [{ kind: "void" }]);
  const rows = operations.filter((row) => row.memberId === "node:worker_threads::MessagePort.start");
  assert.equal(rows.length, 1);
  assert.deepEqual(rows[0].target, { form: "receiver-method", name: "start" });
  assert.equal(rows[0].isFallible, true);
  assert.equal(rows[0].errorBoundary, "provider-native");
  assert.equal(rows[0].errorCarrier.id, "rust.node.NodeError");
  assert.deepEqual(rows[0].parameterCarriers, []);
  assert.deepEqual(rows[0].resultCarrier, { kind: "tuple", elements: [] });
}

for (const [surface, context] of [["native", {}], ["js", { selectedSurfaceIds: ["js"] }]]) {
  test(`${surface} worker port activation keeps source void and transports exact native admission failure`, () => {
    assertPortFallibility(context);
  });

  test(`${surface} Worker terminate owns its native completion error without taxing other Node operations`, () => {
    const [contribution] = createTsonicPlugin().createTargetContributions(context);
    const { operations, carrierPaths, carrierTraits, modules } = contribution.definition;
    const rows = operations.filter(row => row.memberId === "node:worker_threads::Worker.terminate");
    assert.equal(rows.length, 1, "one terminate operation");
    const [completion] = rows;
    assert.equal(completion.isAsync, true, "terminate is still native async");
    assert.equal(completion.isFallible, true, "terminate retains native failure");
    assert.equal(completion.errorBoundary, "provider-native", "native completion boundary");
    assert.equal(completion.errorCarrier.kind, "target-named", "one concrete completion error");
    assert.equal(completion.errorCarrier.id, "rust.node.WorkerCompletionError", "Worker-specific native error");
    assert.equal(completion.resultCarrier.kind, "source-primitive", "unchanged native result");
    assert.equal(completion.resultCarrier.name, "int32", "native exit code remains int32");
    assert.equal(carrierPaths["rust.node.WorkerCompletionError"],
      "tsonic_rust_node::worker_threads::WorkerCompletionError", "canonical public native path");
    const traits = carrierTraits["rust.node.WorkerCompletionError"].implementations;
    assert.equal(traits.some(row => row.trait.path === "core::clone::Clone"), true, "actual native Clone implementation");
    const erasure = traits.find(row => row.trait.path === "core::convert::Into");
    assert.equal(erasure !== undefined, true, "actual retained-error conversion declared");
    assert.equal(erasure.trait.genericArguments.length, 1, "one erasure target");
    assert.equal(erasure.trait.genericArguments[0].kind, "type", "typed erasure target");
    assert.equal(erasure.trait.genericArguments[0].type.id, rustRetainedErrorTargetType().id, "existing retained rejection only");
    assert.equal(traits.some(row => row.trait.path === "tsonic_rust_runtime::ErrorObject"), false,
      "no invented direct ErrorObject implementation on native completion value");
    const ordinary = operations.filter(row => row.exportId === "node:worker_threads::Worker" &&
      row.memberId !== completion.memberId && row.errorBoundary === "provider-native");
    assert.equal(ordinary.length > 0, true, "ordinary Worker operations retained");
    assert.equal(ordinary.every(row => row.errorCarrier.id === "rust.node.NodeError"), true,
      "ordinary Worker operations retain common NodeError");
    const workerModule = modules.find(row => row.moduleSpecifier === "node:worker_threads");
    assert.equal(workerModule.exports.some(row => row.name === "WorkerCompletionError"), false,
      "native observation value does not invent a JS API");
  });
}
