import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";

test("native NodeError exposes its readonly native Error contract", () => {
  const [contribution] = createTsonicPlugin().createTargetContributions({});
  const { modules, types, operations } = contribution.definition;
  const declaration = modules.find(module => module.moduleSpecifier === "node:util")
    .exports.find(entry => entry.name === "NodeError");
  assert.equal(declaration.kind, "interface");
  assert.deepEqual(declaration.heritage, [{ kind: "extends", type: { kind: "source-global", name: "Readonly",
    typeArguments: [{ kind: "source-global", name: "Error" }] } }]);
  assert.deepEqual(declaration.members, [{
    id: "node:util::NodeError.message", name: "message", kind: "property",
    readonly: true, type: { kind: "string" },
  }]);
  assert.equal(types.find(entry => entry.exportId === declaration.id).targetCarrier.id,
    "rust.node.NodeError");
  const rows = operations.filter(entry => entry.exportId === declaration.id);
  assert.equal(rows.length, 1);
  assert.deepEqual(rows[0].target, { form: "receiver-method", name: "message" });
  assert.equal(rows[0].receiverCarrier.id, "rust.node.NodeError");
});

test("compression callbacks preserve independent native error and result absence", () => {
  const [contribution] = createTsonicPlugin().createTargetContributions({});
  const module = contribution.definition.modules.find(entry => entry.moduleSpecifier === "node:zlib");
  for (const name of ["gzip", "gunzip", "deflate", "inflate"]) {
    const signatures = module.exports.find(entry => entry.name === name).signatures;
    assert.equal(signatures.length, 2);
    for (const signature of signatures) {
      const callback = signature.parameters.at(-1).type;
      assert.deepEqual(callback.parameters.map(parameter => parameter.type), [
        { kind: "union", types: [
          { kind: "provider-ref", moduleSpecifier: "node:util", exportName: "NodeError" },
          { kind: "undefined" },
        ] },
        { kind: "union", types: [
          { kind: "provider-ref", moduleSpecifier: "node:buffer", exportName: "Buffer" },
          { kind: "undefined" },
        ] },
      ]);
    }
  }
});
