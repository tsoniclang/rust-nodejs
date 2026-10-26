import assert from "node:assert/strict";
import test from "node:test";
import { fsPromisesModule, fsPromisesRows } from "../../../dist/provider/modules/filesystem/promises.js";

test("filesystem async signatures publish their exact pending Promise payloads before native selection", () => {
  const module = fsPromisesModule();
  const rows = fsPromisesRows();
  const string = { kind: "string" };
  const unit = { kind: "void" };
  const expected = new Map([
    ["readFile(path)", { kind: "provider-ref", moduleSpecifier: "node:buffer", exportName: "Buffer" }],
    ["readFile(path,encoding)", string],
    ["writeFile(path,buffer)", unit],
    ["writeFile(path,string,encoding)", unit],
    ["readdir(path)", { kind: "array", elementType: string }],
    ["stat(path)", { kind: "provider-ref", moduleSpecifier: "node:fs", exportName: "Stats" }],
    ["lstat(path)", { kind: "provider-ref", moduleSpecifier: "node:fs", exportName: "Stats" }],
    ["realpath(path)", string],
    ["mkdir(path)", unit],
    ["mkdir(path,options)", unit],
    ["rm(path)", unit],
    ["rm(path,options)", unit],
    ["unlink(path)", unit],
    ["copyFile(from,to)", unit],
    ["rename(from,to)", unit],
  ]);
  const signatures = module.exports.flatMap(declaration => declaration.signatures);
  assert.equal(signatures.length, expected.size, "every declared overload retains one exact asynchronous result");
  for (const [name, payload] of expected) {
    const id = `node:fs/promises::${name}`;
    const declared = signatures.filter(signature => signature.id === id);
    assert.equal(declared.length, 1, id);
    assert.deepEqual(declared[0].returnType, {
      kind: "source-global", name: "Promise", typeArguments: [payload],
    }, id);
    const operations = rows.filter(row => row.signatureId === id ||
      row.signatureId === undefined && row.exportId === id.slice(0, id.indexOf("(")));
    assert.equal(operations.length, 1, id);
    assert.equal(operations[0].isAsync, true, `${id}: the selected native helper genuinely suspends`);
    assert.equal(operations[0].isFallible, true, id);
    assert.equal(operations[0].errorBoundary, "provider-native", id);
    assert.deepEqual(operations[0].errorCarrier, { kind: "target-named", id: "rust.node.NodeError" }, id);
  }
});
