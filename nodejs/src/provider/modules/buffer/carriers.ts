import { rustJsTypedArrayTargetType, rustNamedTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const bufferCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.Buffer", "tsonic_rust_node::buffer::Buffer", [], [], undefined,
  [{ target: rustJsTypedArrayTargetType("Uint8Array"), path: "tsonic_rust_node::buffer::Buffer::as_uint8_array" }],
);
