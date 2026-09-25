import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const hashCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Hash" };

export const hmacCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Hmac" };

export const cryptoCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Crypto" };
