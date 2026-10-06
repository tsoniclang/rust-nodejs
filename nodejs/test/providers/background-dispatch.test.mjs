import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";

test("native compression callbacks demand their exact component background owner", () => {
  for (const selectedSurfaceIds of [[], ["js"]]) {
    const [contribution] = createTsonicPlugin().createTargetContributions({ selectedSurfaceIds });
    const definition = contribution.definition;
    assert.equal(definition.dispatchContexts.length, 1);
    const context = definition.dispatchContexts[0];
    assert.equal(context.id, "tsonic.rust.node.background");
    assert.equal(context.requiredCrate, "tsonic_rust_node");
    assert.deepEqual(context.construct, { form: "call", path: "tsonic_rust_node::background::BackgroundTasks::new", const: true });
    for (const [role, id] of [["rootCarrier", "rust.node.BackgroundTasks"], ["handleCarrier", "rust.node.BackgroundHandle"]]) {
      assert.deepEqual(context[role], { kind: "target-named", id, genericArguments: [{
        kind: "type", type: { kind: "target-named", id: "rust.program.TsonicError" },
      }] });
      assert.equal(Object.isFrozen(context[role]), true);
    }
    for (const name of ["gzip", "gunzip", "deflate", "inflate"]) {
      const operations = definition.operations.filter(row => row.exportId === `node:zlib::${name}`);
      assert.equal(operations.length, 2);
      for (const operation of operations) {
        assert.deepEqual(operation.dispatchInputs, [{ contextId: context.id, view: "root", targetArgumentIndex: 0, mode: "ref" }]);
        assert.equal(Object.isFrozen(operation.dispatchInputs), true);
        assert.equal(operation.errorBoundary, "provider-native");
      }
    }
    for (const name of ["lookup", "resolve4", "resolve6", "reverse"]) {
      const operations = definition.operations.filter(row => row.exportId === `node:dns::${name}`);
      assert.equal(operations.length, 1);
      assert.deepEqual(operations[0].dispatchInputs, [{ contextId: context.id, view: "root", targetArgumentIndex: 0, mode: "ref" }]);
      assert.equal(Object.isFrozen(operations[0].dispatchInputs), true);
    }
    const synchronous = definition.operations.filter(row => row.exportId === "node:zlib::gzipSync");
    assert.equal(synchronous.length, 2);
    assert.equal(synchronous.every(row => row.dispatchInputs === undefined), true);
  }
});
