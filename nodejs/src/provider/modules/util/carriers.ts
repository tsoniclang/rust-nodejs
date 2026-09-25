import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const textDecoderCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.TextDecoder" };

export const textEncoderCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.TextEncoder" };
