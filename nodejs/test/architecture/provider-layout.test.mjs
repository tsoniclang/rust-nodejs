import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { readSourceInventory } from "../../../../tsonic/test/architecture/tooling/file-inventory.mjs";
import { evaluateNodeProviderContract } from "../../../../tsonic/test/architecture/tooling/node-provider-contract.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

const forbiddenEnginePackages = new Set([
  "boa_engine", "deno_core", "javascriptcore-rs", "libquickjs-sys", "mozjs",
  "quick-js", "quickjs", "quickjs-sys", "rquickjs", "rquickjs-core",
  "rquickjs-sys", "rusty_v8", "v8",
]);

function forbiddenRuntimeDependencies(metadata) {
  assert.ok(Array.isArray(metadata.packages), "Cargo metadata must contain packages");
  return metadata.packages.flatMap((entry) => {
    assert.equal(typeof entry.name, "string", "Cargo packages must carry their actual name");
    return forbiddenEnginePackages.has(entry.name) ? [entry.name] : [];
  }).sort();
}

test("Rust Node's complete locked dependency graph contains no embedded JavaScript engine", () => {
  const result = spawnSync("cargo", ["metadata", "--format-version", "1", "--locked"], {
    cwd: repositoryRoot,
    encoding: "utf8",
    timeout: 60_000,
    maxBuffer: 16 * 1024 * 1024,
  });
  assert.equal(result.status, 0, result.error?.message ?? result.stderr);
  assert.deepEqual(forbiddenRuntimeDependencies(JSON.parse(result.stdout)), []);
});

test("engine dependency checks retain actual package identity through aliases and transitives", () => {
  for (const name of forbiddenEnginePackages) {
    assert.deepEqual(forbiddenRuntimeDependencies({ packages: [
      { name: "tsonic_rust_node", dependencies: [{ name: "indirect", rename: "safe" }] },
      { name: "indirect", dependencies: [{ name, rename: "native_helper" }] },
      { name },
    ] }), [name]);
  }
  assert.deepEqual(forbiddenRuntimeDependencies({ packages: [{ name: "tsonic_rust_node" }] }), []);
  assert.throws(() => forbiddenRuntimeDependencies({}), /must contain packages/u);
  assert.throws(() => forbiddenRuntimeDependencies({ packages: [{}] }), /actual name/u);
});

test("Rust Node provider follows the shared module, ownership and public SDK contract", () => {
  const sources = readSourceInventory(resolve(repositoryRoot, "nodejs/src"), {
    extensions: [".ts"],
  });
  const providerSources = new Map([...sources].map(([path, source]) => [`nodejs/src/${path}`, source]));
  assert.deepEqual(evaluateNodeProviderContract(providerSources, {
    targetPackage: "@tsonic/target-rust",
    factoryName: "createRustProviderPackage",
  }), []);
});
