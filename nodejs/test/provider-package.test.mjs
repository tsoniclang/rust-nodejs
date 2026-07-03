import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../dist/index.js";

const expectedModules = [
  "node:path",
  "node:os",
  "node:fs",
  "node:fs/promises",
  "node:process",
  "node:buffer",
  "node:url",
  "node:crypto",
  "node:util",
];

test("provider package declares the expected node module specifiers", () => {
  const plugin = createTsonicPlugin();
  const specifiers = plugin.providerPackage.moduleOwnership.map((ownership) => ownership.specifierPrefix);
  for (const moduleSpecifier of expectedModules) {
    assert.ok(specifiers.includes(moduleSpecifier), `missing module '${moduleSpecifier}'`);
  }
  assert.equal(specifiers.length, expectedModules.length);
});

test("provider package contributes a non-empty operation row set", () => {
  const plugin = createTsonicPlugin();
  const rows = plugin.providerPackage.rustProviderOperations();
  assert.ok(rows.length > 0);
  const readFileSync = rows.find((row) => row.exportId === "node:fs::readFileSync");
  assert.ok(readFileSync !== undefined, "missing node:fs::readFileSync row");
  assert.equal(readFileSync.isFallible, true);
  assert.equal(readFileSync.operationKind, "method");
});
