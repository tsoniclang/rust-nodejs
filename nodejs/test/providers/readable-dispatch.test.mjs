import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";

const taskContextId = "tsonic.rust.node.runtime-tasks";

function input(contextId, view, targetArgumentIndex, mode) {
  return { contextId, view, targetArgumentIndex, mode };
}

test("every advertised readable factory retains the exact owning typed task handle", () => {
  for (const selectedSurfaceIds of [[], ["js"]]) {
    const [{ definition }] = createTsonicPlugin().createTargetContributions({ selectedSurfaceIds });
    const context = definition.dispatchContexts.find(row => row.id === taskContextId);
    assert.equal(context === undefined, false, "existing task owner");
    assert.deepEqual(context.handle, { form: "receiver-method", name: "handle" });
    assert.deepEqual(context.rootCarrier.genericArguments, context.handleCarrier.genericArguments);
    assert.equal(definition.carrierPaths[context.handleCarrier.id], "tsonic_rust_runtime::dispatch_queue::TaskHandle");
    for (const [exportId, memberId, expected, count] of [
      ["node:stream::Readable", "node:stream::Readable.from", [input(taskContextId, "handle", 0, "value")], 1],
      ["node:fs::createReadStream", undefined, [input("tsonic.rust.node.background", "root", 0, "ref"), input(taskContextId, "handle", 1, "value")], 2],
      ["node:http::createServer", undefined, [input("tsonic.rust.node.http", "root", 0, "ref"), input(taskContextId, "handle", 1, "value")], 1],
      ["node:https::createServer", undefined, [input("tsonic.rust.node.http", "root", 0, "ref"), input("tsonic.rust.node.tls", "root", 1, "ref"), input("tsonic.rust.node.background", "root", 2, "ref"), input(taskContextId, "handle", 3, "value")], 1],
      ...["request", "get"].map(name => [`node:https::${name}`, undefined, [input("tsonic.rust.node.background", "root", 0, "ref"), input(taskContextId, "handle", 1, "value")], 1]),
      ...["Gzip", "Gunzip", "Deflate", "Inflate", "DeflateRaw", "InflateRaw", "BrotliCompress", "BrotliDecompress"].map(name => [`node:zlib::create${name}`, undefined, [input(taskContextId, "handle", 0, "value")], 2]),
    ]) {
      const rows = definition.operations.filter(row => row.exportId === exportId && row.memberId === memberId);
      assert.equal(rows.length, count, exportId);
      for (const row of rows) {
        assert.deepEqual(row.dispatchInputs, expected, row.signatureId ?? exportId);
        assert.equal(Object.isFrozen(row.dispatchInputs), true, exportId);
      }
    }
    const readOptions = definition.modules.find(row => row.moduleSpecifier === "node:stream").exports
      .find(row => row.name === "Readable").members.find(row => row.name === "from");
    assert.equal(readOptions.signatures[0].parameters.length, 1, "no authored scheduler argument");
    for (const carrier of ["Readable", "Duplex", "Transform", "ReadlineOptions"]) {
      assert.equal(definition.carrierTraits[`rust.node.${carrier}`].implementations
        .some(row => row.trait.path === "core::default::Default"), false, carrier);
    }
  }
});

test("named and default stdin accessors select one bounded native input owner and real failure boundary", () => {
  const [{ definition }] = createTsonicPlugin().createTargetContributions({});
  const context = definition.dispatchContexts.find(row => row.id === "tsonic.rust.node.stdin");
  assert.equal(context === undefined, false, "stdin owner");
  assert.deepEqual(context.construct, { form: "call", path: "tsonic_rust_node::process::StdinResources::new", const: true });
  assert.equal(definition.carrierPaths[context.rootCarrier.id], "tsonic_rust_node::process::StdinResources");
  const rows = definition.operations.filter(row => row.exportId === "node:process::stdin" || row.memberId === "node:process::Process.stdin");
  assert.equal(rows.length, 2);
  for (const row of rows) {
    assert.deepEqual(row.dispatchInputs, [input(context.id, "root", 0, "ref"), input("tsonic.rust.node.background", "root", 1, "ref"), input(taskContextId, "handle", 2, "value")]);
    assert.equal(row.isFallible, true);
    assert.equal(row.errorBoundary, "provider-native");
    assert.equal(row.errorCarrier.id, "rust.node.NodeError");
    assert.equal(row.target.path, "node_process::stdin");
    assert.deepEqual(row.resultCarrier.value.genericArguments, context.rootCarrier.genericArguments);
  }
  const options = definition.modules.find(row => row.moduleSpecifier === "node:readline").exports.find(row => row.name === "ReadLineOptions");
  assert.deepEqual(definition.types.find(row => row.exportId === options.id).objectLiteralConstruction, { kind: "struct" });
  assert.equal(options.members.find(row => row.name === "input").optional, undefined);
  for (const name of ["output", "terminal", "prompt"]) {
    assert.equal(options.members.find(row => row.name === name).optional, true, name);
  }
});
