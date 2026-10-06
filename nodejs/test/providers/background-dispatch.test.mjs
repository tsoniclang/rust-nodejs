import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";

test("native compression callbacks demand their exact component background owner", () => {
  for (const selectedSurfaceIds of [[], ["js"]]) {
    const [contribution] = createTsonicPlugin().createTargetContributions({ selectedSurfaceIds });
    const definition = contribution.definition;
    assert.equal(definition.dispatchContexts.length, 9);
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

test("readline callbacks select exact independent background and source-thread task owners", () => {
  for (const selectedSurfaceIds of [[], ["js"]]) {
    const [contribution] = createTsonicPlugin().createTargetContributions({ selectedSurfaceIds });
    const definition = contribution.definition;
    const context = definition.dispatchContexts.find(row => row.id === "tsonic.rust.node.runtime-tasks");
    assert.equal(context !== undefined, true, "exact runtime task owner");
    assert.deepEqual(context.construct, { form: "call", path: "tsonic_rust_node::runtime_tasks::RuntimeTasks::new", const: true });
    assert.equal(context.rootCarrier.id, "rust.node.RuntimeTasks");
    assert.equal(context.handleCarrier.id, "rust.node.RuntimeTaskHandle");
    const operation = definition.operations.find(row => row.memberId === "node:readline::Interface.question");
    assert.equal(operation !== undefined, true, "exact checked source callback operation");
    assert.deepEqual(operation.dispatchInputs, [
      { contextId: "tsonic.rust.node.background", view: "root", targetArgumentIndex: 0, mode: "ref" },
      { contextId: context.id, view: "root", targetArgumentIndex: 1, mode: "ref" },
    ]);
    for (const hook of definition.binaryHooks.filter(row => row.dispatchGroups !== undefined)) {
      assert.deepEqual(hook.dispatchGroups[0].contextIds, ["tsonic.rust.node.background", context.id, "tsonic.rust.node.timers",
        "tsonic.rust.node.workers", "tsonic.rust.node.signals", "tsonic.rust.node.net",
        "tsonic.rust.node.watchers", "tsonic.rust.node.tls", "tsonic.rust.node.http",
        ...(selectedSurfaceIds.includes("js") ? ["tsonic.rust.js.timers"] : [])]);
      assert.equal(Object.isFrozen(hook.dispatchGroups[0].contextIds), true);
    }
  }
});
