import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { createTsonicPlugin } from "../../dist/index.js";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

test("runtime contribution references the canonical package-owned crate", () => {
  const plugin = createTsonicPlugin();
  const contributions = plugin.runtimeContributions({});
  assert.ok(Array.isArray(contributions.references));
  assert.equal(contributions.references.length, 1);
  const reference = contributions.references[0];
  assert.ok(
    reference.include.endsWith(join("rust", "crates", "tsonic_rust_node")),
    `expected canonical crate path, got '${reference.include}'`,
  );
  assert.equal(reference.kind, "cargo-path");
  assert.equal(reference.attributes.crate, "tsonic_rust_node");
  assert.equal(reference.attributes.registryPatch, "crates-io");
});

test("canonical crate path resolves inside this package and exists on disk", () => {
  const plugin = createTsonicPlugin();
  const [reference] = plugin.runtimeContributions({}).references;
  assert.equal(reference.include, resolve(repoRoot, "rust/crates/tsonic_rust_node"));
  assert.ok(existsSync(reference.include), `missing runtime crate at '${reference.include}'`);
  assert.ok(existsSync(join(reference.include, "Cargo.toml")), "packaged crate lacks Cargo.toml");
  assert.ok(existsSync(join(reference.include, "src", "lib.rs")), "packaged crate lacks src/lib.rs");
});
