import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";

test("Buffer range and search parameters retain exact native generic bindings", () => {
  const [contribution] = createTsonicPlugin().createTargetContributions({});
  const operations = contribution.definition.operations;
  for (const [signature, parameters] of [
    ["toString(encoding,start)", ["Start"]],
    ["toString(encoding,start,end)", ["Start", "End"]],
    ["indexOf(byte)", ["Value"]],
    ["indexOf(byte,byteOffset)", ["Value", "ByteOffset"]],
    ["indexOf(buffer,byteOffset)", ["ByteOffset"]],
  ]) {
    const row = operations.find(operation => operation.signatureId === `node:buffer::Buffer.${signature}`);
    assert.ok(row, signature);
    assert.deepEqual(row.genericParameters, parameters.map(name => ({
      kind: "type", targetIdentity: `node:buffer:numeric:${name}`, sourceName: name,
    })), signature);
    assert.deepEqual(row.parameterCarriers.filter(carrier => carrier.kind === "type-parameter"),
      parameters.map(name => ({ kind: "type-parameter", identity: `node:buffer:numeric:${name}`, name })), signature);
    assert.equal(row.isFallible, true, signature);
  }
});

test("Readable pipe preserves its destination binding through parameters, result and requirements", () => {
  const [contribution] = createTsonicPlugin().createTargetContributions({});
  const row = contribution.definition.operations.find(operation =>
    operation.signatureId === "node:stream::Readable.pipe(destination)");
  assert.ok(row);
  const identity = "node:stream:Readable:pipe:TDestination";
  const carrier = { kind: "type-parameter", identity, name: "TDestination" };
  assert.deepEqual(row.genericParameters, [{ kind: "type", targetIdentity: identity, sourceName: "TDestination" }]);
  assert.deepEqual(row.parameterCarriers, [carrier]);
  assert.deepEqual(row.resultCarrier, carrier);
  assert.deepEqual(row.typeRequirements, [{ identity, name: "TDestination", requirements: [
    "clone", { kind: "trait", path: "tsonic_rust_node::stream::WritableTarget", genericArguments: [], associatedConstraints: [] },
  ] }]);
});
