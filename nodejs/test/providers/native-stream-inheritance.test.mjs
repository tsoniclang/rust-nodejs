import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";

test("inherited stream operations retain the declared native receiver and exact base projections", () => {
  const [{ definition }] = createTsonicPlugin().createTargetContributions({});
  const declarations = new Map(definition.modules.flatMap(module =>
    module.exports.map(declaration => [declaration.id, declaration])));
  const carriers = new Map(definition.types.map(row => [row.exportId, row.targetCarrier]));
  for (const [id, parent, carrierId, projections] of [
    ["node:fs::WriteStream", ["node:stream", "Writable"], "rust.node.WriteStream", [
      ["rust.node.Writable", "tsonic_rust_node::fs::write_stream_as_writable"],
    ]],
    ["node:stream::Transform", ["node:stream", "Duplex"], "rust.node.Transform", [
      ["rust.node.Duplex", "tsonic_rust_node::stream::transform_as_duplex"],
    ]],
    ["node:zlib::ZlibTransform", ["node:stream", "Transform"], "rust.node.ZlibTransform", [
      ["rust.node.Duplex", "tsonic_rust_node::zlib::zlib_as_duplex"],
      ["rust.node.Transform", "tsonic_rust_node::zlib::zlib_as_transform"],
    ]],
  ]) {
    const declaration = declarations.get(id);
    assert.ok(declaration, id);
    assert.deepEqual(declaration.heritage, [{
      kind: "extends",
      type: { kind: "provider-ref", moduleSpecifier: parent[0], exportName: parent[1] },
    }], id);
    const carrier = carriers.get(id);
    assert.equal(carrier?.kind, "target-specific", id);
    assert.equal(carrier.target, "rust", id);
    assert.equal(carrier.value.id, carrierId, id);
    for (const [target, path] of projections) {
      assert.equal(carrier.value.upcasts.filter(row => row.target.value.id === target && row.path === path).length, 1, path);
    }
    assert.equal(declaration.members.some(member =>
      ["on", "once", "off", "write", "end", "destroy"].includes(member.name)), false, id);
  }
  const writableRows = definition.operations.filter(row => row.exportId === "node:stream::Writable");
  for (const [signature, name] of [
    ["node:stream::Writable.on.error", "on_error"],
    ["node:stream::Writable.off.error", "off_error"],
    ["node:stream::Writable.once.error", "once_error"],
    ["node:stream::Writable.off.drain", "off_drain"],
    ["node:stream::Writable.once.drain", "once_drain"],
    ["node:stream::Writable.off.finish", "off_finish"],
    ["node:stream::Writable.once.finish", "once_finish"],
    ["node:stream::Writable.write(buffer)", "write_buffer"],
    ["node:stream::Writable.end()", "end"],
  ]) {
    const selected = writableRows.filter(row => row.signatureId === signature);
    assert.equal(selected.length, 1, signature);
    assert.equal(selected[0].target.form, "receiver-method", signature);
    assert.equal(selected[0].target.name, name, signature);
    assert.equal(selected[0].receiverCarrier.value.id, "rust.node.Writable", signature);
  }
  const destroy = definition.operations.filter(row =>
    row.memberId === "node:stream::Duplex.destroy");
  assert.equal(destroy.length, 1);
  assert.equal(destroy[0].target.name, "destroy_chain");
  assert.equal(destroy[0].receiverCarrier.value.id, "rust.node.Duplex");
});

test("advertised duplex lifecycle operations retain exact receivers and finalization fallibility", () => {
  const [{ definition }] = createTsonicPlugin().createTargetContributions({});
  for (const owner of ["Writable", "Duplex"]) {
    const id = `node:stream::${owner}`;
    for (const member of ["end", "uncork"]) {
      const rows = definition.operations.filter(row => row.memberId === `${id}.${member}`);
      assert.ok(rows.length > 0, `${id}.${member}`);
      for (const row of rows) {
        assert.equal(row.isFallible, true, `${id}.${member}`);
        assert.equal(row.errorBoundary, "provider-native", `${id}.${member}`);
        assert.equal(row.errorCarrier.kind, "target-named", `${id}.${member}`);
        assert.equal(row.errorCarrier.id, "rust.node.NodeError", `${id}.${member}`);
        assert.equal(row.receiverCarrier.value.id, `rust.node.${owner}`, `${id}.${member}`);
      }
    }
  }
  for (const method of ["on", "once", "off"]) {
    for (const event of ["drain", "finish", "error", "close"]) {
      const signature = `node:stream::Duplex.${method}.${event}`;
      const rows = definition.operations.filter(row => row.signatureId === signature);
      assert.equal(rows.length, 1, signature);
      assert.equal(rows[0].target.name, `${method}_${event}`, signature);
      assert.equal(rows[0].receiverCarrier.value.id, "rust.node.Duplex", signature);
      assert.equal(rows[0].isFallible, true, signature);
    }
  }
});
