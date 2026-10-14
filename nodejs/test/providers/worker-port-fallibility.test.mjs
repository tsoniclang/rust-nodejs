import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";

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
}
