import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const targetRustRepo = resolve(repoRoot, "../tsonic-rust");

// Simulate the flat npm install layout:
//   node_modules/@tsonic/target-rust   (peer, ships runtimes/crates/*)
//   node_modules/@tsonic/rust-nodejs   (this package)
const installRoot = join(repoRoot, ".temp/installed");
const scopeDir = join(installRoot, "node_modules/@tsonic");
const installedTargetRust = join(scopeDir, "target-rust");
const installedRustNodejs = join(scopeDir, "rust-nodejs");

function copyPackage(sourceRoot, destRoot) {
  mkdirSync(destRoot, { recursive: true });
  for (const entry of ["package.json", "dist", "runtimes"]) {
    const source = join(sourceRoot, entry);
    assert.ok(existsSync(source), `missing packaged entry: ${source}`);
    cpSync(source, join(destRoot, entry), { recursive: true });
  }
}

test("packaged crate dependencies resolve from a flat installed layout", () => {
  rmSync(installRoot, { recursive: true, force: true });
  copyPackage(targetRustRepo, installedTargetRust);
  copyPackage(repoRoot, installedRustNodejs);

  const manifestPath = join(
    installedRustNodejs,
    "runtimes/crates/tsonic_rust_node/Cargo.toml",
  );

  let output;
  try {
    output = execFileSync(
      "cargo",
      ["metadata", "--no-deps", "--offline", "--format-version", "1", "--manifest-path", manifestPath],
      { encoding: "utf8" },
    );
  } catch (error) {
    assert.fail(
      `cargo metadata failed against the installed layout; peer-layout path deps do not resolve\n${error.stderr ?? error.message}`,
    );
  }

  const metadata = JSON.parse(output);
  const pkg = metadata.packages.find((entry) => entry.name === "tsonic_rust_node");
  assert.ok(pkg, "cargo metadata did not report tsonic_rust_node");
  for (const dep of ["tsonic_rust_js", "tsonic_rust_runtime"]) {
    const declared = pkg.dependencies.find((entry) => entry.name === dep);
    assert.ok(declared, `packaged crate must depend on ${dep}`);
    assert.ok(declared.path, `${dep} must be a path dependency`);
    assert.ok(
      existsSync(join(declared.path, "Cargo.toml")),
      `${dep} path dependency must resolve to a crate inside the installed peer: ${declared.path}`,
    );
  }
});
