import { nodeProgramErrorArguments } from "../../model/carriers.js";
import { rustNamedTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const streamCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.Stream",
  "tsonic_rust_node::stream::Stream",
  nodeProgramErrorArguments,
);

export const readableCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.Readable",
  "tsonic_rust_node::stream::Readable",
  nodeProgramErrorArguments,
  [],
  undefined,
  [{ target: streamCarrier, path: "tsonic_rust_node::stream::readable_as_stream" }],
);

export const writableCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.Writable",
  "tsonic_rust_node::stream::Writable",
  nodeProgramErrorArguments,
  [],
  undefined,
  [{ target: streamCarrier, path: "tsonic_rust_node::stream::writable_as_stream" }],
);

export const duplexCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.Duplex",
  "tsonic_rust_node::stream::Duplex",
  nodeProgramErrorArguments,
  [],
  undefined,
  [
    { target: streamCarrier, path: "tsonic_rust_node::stream::duplex_as_stream" },
    { target: readableCarrier, path: "tsonic_rust_node::stream::duplex_as_readable" },
    { target: writableCarrier, path: "tsonic_rust_node::stream::duplex_as_writable" },
  ],
);

export const transformCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.Transform",
  "tsonic_rust_node::stream::Transform",
  nodeProgramErrorArguments,
  [],
  undefined,
  [
    { target: streamCarrier, path: "tsonic_rust_node::stream::transform_as_stream" },
    { target: readableCarrier, path: "tsonic_rust_node::stream::transform_as_readable" },
    { target: writableCarrier, path: "tsonic_rust_node::stream::transform_as_writable" },
    { target: duplexCarrier, path: "tsonic_rust_node::stream::transform_as_duplex" },
  ],
);
