import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";

test("timer cancellation retains exact Timeout declarations and native operations", () => {
  for (const selectedSurfaceIds of [[], ["js"]]) {
    const [contribution] = createTsonicPlugin().createTargetContributions({ selectedSurfaceIds });
    const definition = contribution.definition;
    const module = definition.modules.find(row => row.moduleSpecifier === "node:timers");
    assert.equal(module !== undefined, true, "canonical timer module");
    for (const name of ["clearTimeout", "clearInterval"]) {
      const declaration = module.exports.find(row => row.name === name);
      assert.equal(declaration.signatures.length, 1);
      assert.deepEqual(declaration.signatures[0].parameters, [{ name: "timeout", type: {
        kind: "provider-ref", moduleSpecifier: "node:timers", exportName: "Timeout",
      } }]);
      assert.deepEqual(declaration.signatures[0].returnType, { kind: "void" });
      const operations = definition.operations.filter(row => row.exportId === `node:timers::${name}`);
      assert.equal(operations.length, 1);
      assert.deepEqual(operations[0].target, { form: "call",
        path: `node_timers::${name === "clearTimeout" ? "clear_timeout" : "clear_interval"}`, argModes: ["mut-ref"] });
      assert.equal(operations[0].parameterCarriers[0].id, "rust.node.Timeout");
      assert.deepEqual(operations[0].resultCarrier, { kind: "tuple", elements: [] });
      assert.equal(Object.isFrozen(operations[0]), true);
      assert.equal(Object.isFrozen(operations[0].target), true);
      assert.equal(Object.isFrozen(operations[0].parameterCarriers), true);
    }
  }
});
