import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../dist/index.js";

const [contribution] = createTsonicPlugin().createTargetContributions({});
const operations = contribution.definition.operations;

test("native counter and binary results retain their actual widths without result adapters", () => {
  const contracts = [
    ["node:fs::Stats.size", "uint64"],
    ["node:fs::Stats.mode", "uint32"],
    ["node:fs::ReadStream.bytesRead", "native-uint"],
    ["node:fs::WriteStream.bytesWritten", "native-uint"],
    ["node:process::availableMemory", "uint64"],
    ["node:process::constrainedMemory", "uint64"],
    ["node:process::pid", "uint32"],
    ["node:process::ppid", "uint32"],
    ["node:process::CpuUsage.user", "int64"],
    ["node:process::CpuUsage.system", "int64"],
    ["node:buffer::Buffer.length", "native-uint"],
    ["node:buffer::Buffer.byteLength", "native-uint"],
    ["node:buffer::Buffer.compare", "int32"],
    ["node:net::isIP", "uint8"],
    ...["rss", "heapTotal", "heapUsed", "external", "arrayBuffers"].map(name =>
      [`node:process::MemoryUsage.${name}`, "uint64"]),
    ...[
      ["readUInt8", "uint8"], ["readInt8", "int8"],
      ["readUInt16LE", "uint16"], ["readUInt16BE", "uint16"],
      ["readInt16LE", "int16"], ["readInt16BE", "int16"],
      ["readUInt32LE", "uint32"], ["readUInt32BE", "uint32"],
      ["readInt32LE", "int32"], ["readInt32BE", "int32"],
      ["readFloatLE", "float32"], ["readFloatBE", "float32"],
      ["readDoubleLE", "float64"], ["readDoubleBE", "float64"],
    ].map(([name, carrier]) => [`node:buffer::Buffer.${name}`, carrier]),
  ];
  for (const [identity, carrier] of contracts) {
    const selected = operations.filter(row => (row.memberId ?? row.exportId) === identity &&
      row.operationKind !== "set" && row.operationKind !== "property-set");
    assert(selected.length > 0, `missing ${identity}`);
    for (const row of selected) {
      assert.deepEqual(row.resultCarrier, { kind: "source-primitive", name: carrier }, identity);
      assert.equal(row.resultConversion, undefined, identity);
    }
  }
});

test("descriptor counts and high-resolution time never require floating storage", () => {
  for (const name of ["openSync", "readSync", "writeSync"]) {
    const selected = operations.filter(row => row.exportId === `node:fs::${name}`);
    assert(selected.length > 0, name);
    for (const row of selected) {
      assert.deepEqual(row.resultCarrier, {
        kind: "source-primitive", name: name === "openSync" ? "int32" : "native-uint",
      });
      assert.equal(row.resultConversion, undefined);
    }
  }
  const times = operations.filter(row => row.target.path === "node_process::hrtime_open" ||
    row.target.path === "node_process::hrtime_since");
  assert.equal(times.length, 4);
  for (const row of times) {
    assert.deepEqual(row.resultCarrier.genericArguments, [{ kind: "type", type: { kind: "source-primitive", name: "int64" } }]);
    assert.equal(row.isFallible, true);
  }
});

test("fractional process and performance clocks keep floating results", () => {
  for (const identity of ["node:process::uptime", "node:process::Process.uptime"]) {
    const row = operations.find(row => (row.memberId ?? row.exportId) === identity);
    assert(row, identity);
    assert.deepEqual(row.resultCarrier, { kind: "source-primitive", name: "float64" });
  }
});
