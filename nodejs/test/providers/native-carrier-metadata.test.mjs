import assert from "node:assert/strict";
import test from "node:test";
import { bufferCarrier } from "../../../dist/provider/modules/buffer/carriers.js";
import { readStreamCarrier, writeStreamCarrier } from "../../../dist/provider/modules/filesystem/carriers.js";
import { incomingHttpHeadersCarrier, outgoingHttpHeadersCarrier, httpAddressInfoCarrier,
  httpServerAddressCarrier, httpIncomingMessageCarrier, httpServerResponseCarrier } from "../../../dist/provider/modules/http/carriers.js";
import { streamCarrier, readableCarrier, writableCarrier, duplexCarrier, transformCarrier }
  from "../../../dist/provider/modules/stream/carriers.js";
import { zlibTransformCarrier, zlibTransformTraits } from "../../../dist/provider/modules/zlib/carriers.js";

for (const [carrier, path, arity, upcasts] of [
  [bufferCarrier, "buffer::Buffer", 0, 1],
  [incomingHttpHeadersCarrier, "http::IncomingHttpHeaders", 0, 0],
  [outgoingHttpHeadersCarrier, "http::OutgoingHttpHeaders", 0, 0],
  [httpAddressInfoCarrier, "http::AddressInfo", 0, 0],
  [httpServerAddressCarrier, "http::ServerAddress", 0, 0],
  [streamCarrier, "stream::Stream", 1, 0],
  [readableCarrier, "stream::Readable", 1, 1],
  [writableCarrier, "stream::Writable", 1, 1],
  [duplexCarrier, "stream::Duplex", 1, 3],
  [transformCarrier, "stream::Transform", 1, 4],
  [readStreamCarrier, "fs::ReadStream", 1, 2],
  [writeStreamCarrier, "fs::WriteStream", 1, 2],
  [httpIncomingMessageCarrier, "http::IncomingMessage", 1, 2],
  [httpServerResponseCarrier, "http::ServerResponse", 1, 2],
  [zlibTransformCarrier, "zlib::Zlib", 1, 5],
]) test(`${path} retains exact current native carrier metadata`, () => {
  assert.equal(carrier.kind, "target-specific", path);
  assert.equal(carrier.target, "rust", path);
  assert.equal(carrier.name, "named-type", path);
  const native = carrier.value;
  assert.equal(native.path, `tsonic_rust_node::${path}`, path);
  assert.equal(native.genericArguments.length, arity, `${path}: complete native argument vector`);
  assert.equal(native.nativeArgumentCount, arity, `${path}: no native argument is silently omitted`);
  assert.equal(Array.isArray(native.typeRequirements) && native.typeRequirements.length === 0, true,
    `${path}: the actual native declaration has no TYPE trait requirements`);
  assert.equal(Object.hasOwn(native, "nativeErrorIdentity"), false,
    `${path}: an ordinary native type is not an error identity`);
  assert.equal(native.upcasts.length, upcasts, `${path}: all original projections remain`);
  if (arity === 1) {
    const argument = native.genericArguments[0];
    assert.equal(argument.kind, "type", path);
    assert.equal(argument.type.id, "rust.program.TsonicError", `${path}: original selected program error`);
    assert.equal(argument.type.errorOwner.kind, "logical", `${path}: physical error selection stays with its owner`);
  }
  if (carrier === zlibTransformCarrier)
    assert.equal(native.traits === zlibTransformTraits, true, "Zlib retains its original native trait contract");
});
