import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin, createRustNodejsCapability } from "../../dist/index.js";

test("createTsonicPlugin returns the Rust NodeJS target capability", () => {
  const plugin = createTsonicPlugin();
  assert.equal(plugin.kind, "target-capability");
  assert.equal(plugin.id, "@tsonic/rust-nodejs");
  assert.equal(plugin.targetId, "rust");
  assert.equal(plugin.displayName, "Node.js for Rust");
  assert.deepEqual(plugin.moduleOwnership, [
    "node:path",
    "node:os",
    "node:fs",
    "node:fs/promises",
    "node:process",
    "node:buffer",
    "node:url",
    "node:crypto",
    "node:util",
  ].map((specifierPrefix) => ({
    specifierPrefix,
    providerId: "tsonic.rust.provider-package.@tsonic/rust-nodejs.binding",
  })));
  assert.deepEqual(plugin.requiredSurfaces, ["js"]);
});

test("plugin exposes source, target-policy, and runtime contributions", () => {
  const plugin = createRustNodejsCapability();
  assert.equal(typeof plugin.sourceCompilerContributions, "function");
  assert.equal(typeof plugin.createTargetContributions, "function");
  assert.equal(typeof plugin.runtimeContributions, "function");
  const source = plugin.sourceCompilerContributions({});
  assert.equal(source.extensions.length, 1);
  assert.equal(source.extensions[0].identity.id, "tsonic.rust.provider-package.@tsonic/rust-nodejs");
  const [policy] = plugin.createTargetContributions({});
  assert.equal(policy.kind, "rust-provider-policy");
  assert.equal(policy.definition.id, plugin.id);
});
