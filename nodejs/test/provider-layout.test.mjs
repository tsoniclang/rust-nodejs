import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  evaluateBarrelModules,
  formatArchitectureFindings,
} from "../../../tsonic/test/architecture/tooling/architecture-rules.mjs";
import {
  readSourceInventory,
} from "../../../tsonic/test/architecture/tooling/file-inventory.mjs";
import {
  buildTypeScriptModuleAnalysis,
} from "../../../tsonic/test/architecture/tooling/module-graph.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const providerRoot = join(repositoryRoot, "nodejs/src/provider");
const moduleRoot = join(providerRoot, "modules");

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

test("Rust Node provider declarations are owned by semantic modules", () => {
  assert.equal(existsSync(join(providerRoot, "nodejs-package.ts")), false);
  assert.deepEqual(
    readdirSync(providerRoot).sort(),
    ["model.ts", "modules", "package.ts"],
  );
  assert.deepEqual(
    readdirSync(moduleRoot).sort(),
    [
      "assert.ts",
      "buffer.ts",
      "child-process.ts",
      "crypto.ts",
      "dns.ts",
      "events.ts",
      "filesystem-descriptors.ts",
      "filesystem-paths.ts",
      "filesystem-promises.ts",
      "filesystem-realpath.ts",
      "filesystem.ts",
      "http.ts",
      "https.ts",
      "net.ts",
      "os.ts",
      "path.ts",
      "performance.ts",
      "process-metrics.ts",
      "process-signals.ts",
      "process.ts",
      "readline.ts",
      "stream.ts",
      "timers.ts",
      "tls.ts",
      "url.ts",
      "util.ts",
      "v8.ts",
      "worker-threads.ts",
      "zlib.ts",
    ],
  );
});

test("Rust Node package assembly contains no module declaration policy", () => {
  const assembly = readFileSync(join(providerRoot, "package.ts"), "utf8");
  assert.doesNotMatch(assembly, /providerModuleId|RustProviderModuleDefinition/u);
  const fragments = new Map([
    ["filesystem-descriptors.ts", "filesystem.ts"],
    ["filesystem-paths.ts", "filesystem.ts"],
    ["filesystem-realpath.ts", "filesystem.ts"],
    ["process-metrics.ts", "process.ts"],
    ["process-signals.ts", "process.ts"],
  ]);
  for (const moduleFile of readdirSync(moduleRoot)) {
    const source = readFileSync(join(moduleRoot, moduleFile), "utf8");
    const owner = fragments.get(moduleFile);
    if (owner === undefined) {
      assert.match(source, /providerModuleId/u, moduleFile);
    } else {
      assert.doesNotMatch(source, /providerModuleId/u, moduleFile);
      const ownerSource = readFileSync(join(moduleRoot, owner), "utf8");
      assert.ok(ownerSource.includes(`from "./${moduleFile.slice(0, -3)}.js"`), `${moduleFile} has no owning module import`);
    }
    assert.ok(source.split("\n").length <= 600, `${moduleFile} exceeds 600 lines`);
  }
});

test("Rust Node capability composes only the canonical package owner", () => {
  const capability = readFileSync(
    join(repositoryRoot, "nodejs/src/capability.ts"),
    "utf8",
  );
  assert.match(capability, /from "\.\/provider\/package\.js"/u);
  assert.doesNotMatch(capability, /nodejs-package/u);
});

test("Rust Node provider indexes are barrels and target imports use its provider API", () => {
  const sources = readSourceInventory(repositoryRoot, {
    extensions: [".ts"],
    exclude: ["dist", "node_modules", ".analysis", ".temp"],
  });
  const modules = buildTypeScriptModuleAnalysis(sources);
  const findings = evaluateBarrelModules(modules.modules, {
    allowedImplementationFiles: new Set(["nodejs/src/index.ts"]),
  });
  assert.deepEqual(findings, [], formatArchitectureFindings(findings));
  assert.deepEqual(
    modules.edges
      .filter((edge) =>
        edge.kind === "package" &&
        (edge.specifier === "@tsonic/target-rust" ||
          edge.specifier.startsWith("@tsonic/target-rust/")) &&
        edge.specifier !== "@tsonic/target-rust/provider"
      )
      .map((edge) => `${edge.source}: ${edge.specifier}`),
    [],
  );
});
