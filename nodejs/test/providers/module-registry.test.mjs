import assert from "node:assert/strict";
import { isDeepStrictEqual } from "node:util";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";
import { nodejsProviderModules } from "../../../dist/provider/assembly/modules.js";
import { nodejsProviderModuleSpecifiers } from "../../../dist/provider/declarations/specifiers.js";

const expectedCanonicalModules = [
  "node:assert", "node:buffer", "node:child_process", "node:crypto", "node:dns",
  "node:dns/promises", "node:events", "node:fs", "node:fs/promises", "node:http",
  "node:https", "node:net", "node:os", "node:path", "node:perf_hooks", "node:process",
  "node:readline", "node:stream", "node:timers", "node:tls", "node:url", "node:util",
  "node:v8", "node:worker_threads", "node:zlib",
];

test("Node module assembly owns the exact canonical registry on both source profiles", () => {
  const plugin = createTsonicPlugin();
  for (const typedArrays of [false, true]) {
    const modules = nodejsProviderModules(typedArrays);
    assert.deepEqual(modules.map((module) => module.moduleSpecifier).sort(), expectedCanonicalModules);
    assert.equal(new Set(modules.map((module) => module.providerModuleId)).size, modules.length);
    const [contribution] = plugin.createTargetContributions({ selectedSurfaceIds: typedArrays ? ["js"] : [] });
    assert.equal(contribution.kind, "rust-provider-policy");
    assert.equal(isDeepStrictEqual(contribution.definition.modules, modules), true, typedArrays ? "JS profile" : "native profile");
    for (const module of modules) {
      assert.equal(new Set(module.exports.map((entry) => entry.id)).size, module.exports.length, module.moduleSpecifier);
    }
  }
});

test("Node module aliases resolve to one registered canonical declaration owner", () => {
  const plugin = createTsonicPlugin();
  const aliases = nodejsProviderModuleSpecifiers();
  assert.equal(new Set(aliases.map((alias) => alias.moduleSpecifier)).size, aliases.length);
  for (const typedArrays of [false, true]) {
    const modules = nodejsProviderModules(typedArrays);
    const canonicalModules = new Map(modules.map((module) => [module.moduleSpecifier, module]));
    const [contribution] = plugin.createTargetContributions({ selectedSurfaceIds: typedArrays ? ["js"] : [] });
    assert.deepEqual(contribution.definition.moduleAliases, aliases);
    const ownedSpecifiers = [...modules.map((module) => module.moduleSpecifier), ...aliases.map((alias) => alias.moduleSpecifier)];
    assert.equal(new Set(ownedSpecifiers).size, ownedSpecifiers.length);
    assert.deepEqual(plugin.moduleOwnership.map((ownership) => ownership.specifierPrefix), ownedSpecifiers);
    for (const alias of aliases) {
      assert.equal(canonicalModules.has(alias.canonicalModuleSpecifier), true, alias.moduleSpecifier);
    }
    assert.equal(ownedSpecifiers.includes("node:unregistered"), false);
  }
});
