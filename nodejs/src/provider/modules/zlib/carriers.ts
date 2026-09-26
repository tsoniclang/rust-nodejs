import { rustCallableTargetType, rustNamedTargetType, rustOptionTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { bufferCarrier } from "../buffer/carriers.js";
import { nodeErrorCarrier, unitCarrier } from "../../model/carriers.js";
import { duplexCarrier, readableCarrier, streamCarrier, transformCarrier, writableCarrier } from "../stream/carriers.js";

export const zlibOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ZlibOptions" };

export const brotliOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.BrotliOptions" };

export const zlibTransformCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.ZlibTransform",
  "tsonic_rust_node::zlib::Zlib",
  [],
  [],
  undefined,
  [
    { target: streamCarrier, path: "tsonic_rust_node::zlib::zlib_as_stream" },
    { target: readableCarrier, path: "tsonic_rust_node::zlib::zlib_as_readable" },
    { target: writableCarrier, path: "tsonic_rust_node::zlib::zlib_as_writable" },
    { target: duplexCarrier, path: "tsonic_rust_node::zlib::zlib_as_duplex" },
    { target: transformCarrier, path: "tsonic_rust_node::zlib::zlib_as_transform" },
  ],
);

export const zlibCallbackCarrier = rustCallableTargetType(
  [rustOptionTargetType(nodeErrorCarrier), bufferCarrier],
  unitCarrier,
);
