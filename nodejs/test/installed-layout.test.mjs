import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const scratchRoot = resolve(repoRoot, ".temp");
const packageRoots = new Map([
  ["tsts", resolve(repoRoot, "../tsonic/packages/tsts")],
  ["target-api", resolve(repoRoot, "../tsonic/packages/target-api")],
  ["source-core", resolve(repoRoot, "../tsonic/packages/source-core")],
  ["target-rust", resolve(repoRoot, "../tsonic-rust")],
  ["rust-runtime", resolve(repoRoot, "../rust-runtime")],
  ["rust-js", resolve(repoRoot, "../rust-js")],
  ["rust-nodejs", repoRoot],
]);
let packedArtifacts;

function packArtifacts() {
  if (packedArtifacts !== undefined) {
    return packedArtifacts;
  }
  mkdirSync(scratchRoot, { recursive: true });
  const root = mkdtempSync(join(scratchRoot, "npm-pack-layout-"));
  const tarballRoot = join(root, "tarballs");
  mkdirSync(tarballRoot, { recursive: true });
  const packages = new Map();
  for (const [name, sourceRoot] of packageRoots) {
    const output = execFileSync(
      "npm",
      ["pack", sourceRoot, "--ignore-scripts", "--json", "--pack-destination", tarballRoot],
      { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    );
    const records = JSON.parse(output);
    assert.equal(records.length, 1, `${name} must produce exactly one npm artifact`);
    const record = records[0];
    const tarball = join(tarballRoot, record.filename);
    assert.ok(existsSync(tarball), `missing npm artifact ${tarball}`);
    packages.set(name, {
      tarball,
      files: new Set(record.files.map((entry) => entry.path)),
    });
  }
  assertPackInventory(packages);
  packedArtifacts = { root, packages };
  return packedArtifacts;
}

function assertPackInventory(packages) {
  const targetFiles = packages.get("target-rust").files;
  assert.ok(targetFiles.has("dist/index.js"));
  assert.ok(!targetFiles.has("dist/session/compile-input.js"));
  assert.ok(![...targetFiles].some((path) => path.startsWith("runtimes/")));

  const runtimeFiles = packages.get("rust-runtime").files;
  assert.ok(runtimeFiles.has("crates/tsonic_rust_runtime/Cargo.toml"));
  assert.ok(runtimeFiles.has("crates/tsonic_rust_runtime/src/lib.rs"));
  assert.ok(runtimeFiles.has("tests/runtime_integration_tests.rs"));
  assert.ok(!runtimeFiles.has("Cargo.toml"));
  assert.ok(!runtimeFiles.has("Cargo.lock"));

  const jsFiles = packages.get("rust-js").files;
  assert.ok(jsFiles.has("crates/tsonic_rust_js/Cargo.toml"));
  assert.ok(jsFiles.has("crates/tsonic_rust_js/src/lib.rs"));
  assert.ok(jsFiles.has("tests/js_integration_tests.rs"));
  assert.ok(!jsFiles.has("Cargo.toml"));
  assert.ok(!jsFiles.has("Cargo.lock"));

  const nodeFiles = packages.get("rust-nodejs").files;
  assert.ok(nodeFiles.has("dist/index.js"));
  assert.ok(nodeFiles.has("rust/crates/tsonic_rust_node/Cargo.toml"));
  assert.ok(nodeFiles.has("rust/crates/tsonic_rust_node/src/lib.rs"));
  assert.ok(nodeFiles.has("rust/tests/node_integration_tests.rs"));
  assert.ok(!nodeFiles.has("Cargo.toml"));
  assert.ok(!nodeFiles.has("Cargo.lock"));
  assert.ok(![...nodeFiles].some((path) => path.startsWith("runtimes/")));
}

function createApplication(label, { nestedNode = false } = {}) {
  const packed = packArtifacts();
  const root = mkdtempSync(join(scratchRoot, `${label}-`));
  writeFileSync(join(root, "Cargo.toml"), "[workspace]\nmembers = []\n");
  const applicationRoot = join(root, "application");
  mkdirSync(applicationRoot, { recursive: true });
  writeFileSync(join(applicationRoot, "package.json"), JSON.stringify({ private: true }, null, 2));
  const outerTarballs = [...packed.packages.entries()]
    .filter(([name]) => !nestedNode || name !== "rust-nodejs")
    .map(([, entry]) => entry.tarball);
  npmInstall(applicationRoot, outerTarballs);

  let nodePackageRoot = join(applicationRoot, "node_modules/@tsonic/rust-nodejs");
  if (nestedNode) {
    const pluginRoot = join(applicationRoot, "plugins/node-capability");
    mkdirSync(pluginRoot, { recursive: true });
    writeFileSync(join(pluginRoot, "package.json"), JSON.stringify({ private: true }, null, 2));
    npmInstall(pluginRoot, [packed.packages.get("rust-nodejs").tarball], ["--legacy-peer-deps"]);
    nodePackageRoot = join(pluginRoot, "node_modules/@tsonic/rust-nodejs");
  }
  return { root, applicationRoot, nodePackageRoot };
}

function npmInstall(root, tarballs, extraArguments = []) {
  execFileSync(
    "npm",
    [
      "install",
      "--ignore-scripts",
      "--offline",
      "--no-audit",
      "--no-fund",
      "--package-lock=false",
      ...extraArguments,
      ...tarballs,
    ],
    { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  );
}

async function generateInstalledProject(applicationRoot, nodePackageRoot) {
  const scopeRoot = join(applicationRoot, "node_modules/@tsonic");
  const targetModule = await import(pathToFileURL(join(scopeRoot, "target-rust/dist/index.js")).href);
  const nodeModule = await import(pathToFileURL(join(nodePackageRoot, "dist/index.js")).href);
  const targetPlugin = targetModule.createTsonicPlugin();
  const targetPack = targetPlugin.createTargetPack();
  const nodeCapability = nodeModule.createTsonicPlugin();
  const jsSurface = targetPack.surfaces.find((surface) => surface.id === "js");
  assert.ok(jsSurface);

  const projectRoot = join(applicationRoot, "generated");
  mkdirSync(projectRoot, { recursive: true });
  const target = { id: "rust", options: {} };
  const project = { entryPoint: "src/index.ts", targets: [target] };
  const contributionContext = {
    project,
    target,
    selectedPackages: [],
    selectedSurfaces: [jsSurface],
    selectedCapabilities: [nodeCapability],
    paths: {
      projectFilePath: join(projectRoot, "tsonic.json"),
      projectRoot,
      outputRoot: join(projectRoot, "out"),
      targetOutputRoot: join(projectRoot, "out/rust"),
    },
  };
  const runtimeReferences = [
    ...targetPack.provider.runtimeContributions(contributionContext).references,
    ...jsSurface.runtimeContributions(contributionContext).references,
    ...nodeCapability.runtimeContributions(contributionContext).references,
  ];
  const backend = targetPack.createBackend({ project, target });
  const result = backend.compile({
    program: {},
    ast: {},
    types: {},
    sourceFiles: [],
    facts: {},
    analysis: {},
    targetFacts: {},
    project,
    target,
    runtimeReferences,
    paths: contributionContext.paths,
  });
  assert.deepEqual(result.diagnostics, []);
  assert.ok(result.artifacts.length >= 2);
  for (const artifact of result.artifacts) {
    const path = resolve(projectRoot, artifact.path);
    assert.ok(path.startsWith(`${resolve(projectRoot)}${sep}`), `artifact escaped project root: ${artifact.path}`);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, artifact.text);
  }
  return { projectRoot, result };
}

function validateCargoProject(projectRoot, installationRoot, { check }) {
  const manifestPath = join(projectRoot, "Cargo.toml");
  execFileSync("cargo", ["generate-lockfile", "--offline", "--manifest-path", manifestPath], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  const metadataText = execFileSync(
    "cargo",
    ["metadata", "--format-version", "1", "--locked", "--offline", "--manifest-path", manifestPath],
    { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  );
  if (check) {
    execFileSync(
      "cargo",
      ["check", "--all-targets", "--locked", "--offline", "--manifest-path", manifestPath],
      { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    );
  }
  const packages = JSON.parse(metadataText).packages;
  for (const crate of ["tsonic_rust_runtime", "tsonic_rust_js", "tsonic_rust_node"]) {
    const matches = packages.filter((entry) => entry.name === crate);
    assert.equal(matches.length, 1, `${crate} must resolve exactly once`);
    const packageRoot = resolve(dirname(matches[0].manifest_path));
    assert.ok(packageRoot.startsWith(`${resolve(installationRoot)}${sep}`), `${crate} escaped the npm installation`);
    const npmPackageRoot = findOwningNpmPackage(packageRoot, installationRoot);
    for (const target of matches[0].targets) {
      assert.ok(existsSync(target.src_path), `packed Cargo target is missing: ${target.src_path}`);
      assert.ok(resolve(target.src_path).startsWith(`${npmPackageRoot}${sep}`), `Cargo target escaped ${crate}'s npm artifact: ${target.src_path}`);
    }
  }
  assert.ok(existsSync(join(projectRoot, "Cargo.lock")));
}

function findOwningNpmPackage(start, installationRoot) {
  let current = resolve(start);
  const boundary = resolve(installationRoot);
  while (current.startsWith(`${boundary}${sep}`)) {
    const manifestPath = join(current, "package.json");
    if (existsSync(manifestPath)) {
      const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
      if (typeof manifest.name === "string" && manifest.name.startsWith("@tsonic/")) {
        return current;
      }
    }
    const parent = dirname(current);
    if (parent === current) {
      break;
    }
    current = parent;
  }
  assert.fail(`Cargo crate at ${start} has no owning installed npm package`);
}

test("real npm artifacts produce one buildable installed Cargo graph", { timeout: 600_000 }, async () => {
  const installation = createApplication("installed-standard");
  const generated = await generateInstalledProject(installation.applicationRoot, installation.nodePackageRoot);
  const manifest = readFileSync(join(generated.projectRoot, "Cargo.toml"), "utf8");
  for (const crate of ["tsonic_rust_runtime", "tsonic_rust_js", "tsonic_rust_node"]) {
    assert.equal(manifest.split("\n").filter((line) => line.startsWith(`${crate} = `)).length, 2,
      `${crate} must have one dependency and one explicit registry patch`);
  }
  validateCargoProject(generated.projectRoot, installation.applicationRoot, { check: true });
});

test("real npm artifacts resolve a nested capability without sibling assumptions", { timeout: 600_000 }, async () => {
  const installation = createApplication("installed-nested", { nestedNode: true });
  const generated = await generateInstalledProject(installation.applicationRoot, installation.nodePackageRoot);
  validateCargoProject(generated.projectRoot, installation.applicationRoot, { check: false });
});
