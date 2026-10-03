import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";

test("HTTP header indexer exposes an exact native readonly slice borrowed from its owner", () => {
  const [contribution] = createTsonicPlugin().createTargetContributions({});
  const { modules, operations } = contribution.definition;
  const declaration = modules.find(module => module.moduleSpecifier === "node:http")
    .exports.find(entry => entry.name === "IncomingHttpHeaderValues");
  assert.deepEqual(declaration.members[0].signatures[0].returnType, {
    kind: "union", types: [{ kind: "source-global", name: "ReadonlyArray", typeArguments: [{ kind: "string" }] }, { kind: "undefined" }],
  });
  const row = operations.find(operation => operation.exportId === declaration.id && operation.operationKind === "indexer");
  assert.equal(row.target.name, "get_values");
  const reference = row.resultCarrier.genericArguments[0].type;
  assert.equal(reference.kind, "reference");
  assert.equal(reference.mutable, false);
  assert.equal(reference.referent.kind, "slice");
  const snapshot = operations.find(operation => operation.target.name === "get_all" && operation.exportId === "node:http::IncomingHttpHeaders");
  assert.equal(snapshot.resultCarrier.id, "rust.js.JsArray");
});
