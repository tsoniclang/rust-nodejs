import assert from "node:assert/strict";
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
      "crypto.ts",
      "filesystem-promises.ts",
      "filesystem.ts",
      "http.ts",
      "os.ts",
      "path.ts",
      "process.ts",
      "timers.ts",
      "url.ts",
      "util.ts",
    ],
  );
});

test("Rust Node package assembly contains no module declaration policy", () => {
  const assembly = readFileSync(join(providerRoot, "package.ts"), "utf8");
  assert.doesNotMatch(assembly, /providerModuleId|RustProviderModuleDefinition/u);
  for (const moduleFile of readdirSync(moduleRoot)) {
    const source = readFileSync(join(moduleRoot, moduleFile), "utf8");
    assert.match(source, /providerModuleId/u, moduleFile);
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
