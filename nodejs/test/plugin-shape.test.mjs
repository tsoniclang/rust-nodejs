import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin, createRustNodejsCapability } from "../../dist/index.js";

test("createTsonicPlugin returns the Rust NodeJS target capability", () => {
  const plugin = createTsonicPlugin();
  assert.equal(plugin.kind, "target-capability");
  assert.equal(plugin.id, "@tsonic/rust-nodejs");
  assert.equal(plugin.targetId, "rust");
  assert.equal(plugin.displayName, "Node.js for Rust");
  assert.deepEqual(plugin.moduleOwnership, [{ specifierPrefix: "node:" }]);
  assert.deepEqual(plugin.requiredSurfaces, ["js"]);
});

test("plugin exposes delegating extension and runtime hooks", () => {
  const plugin = createRustNodejsCapability();
  assert.equal(typeof plugin.createExtensions, "function");
  assert.equal(typeof plugin.runtimeContributions, "function");
  assert.equal(typeof plugin.providerPackage, "object");
  const extensions = plugin.createExtensions({});
  assert.ok(Array.isArray(extensions));
  assert.ok(extensions.length > 0);
});
