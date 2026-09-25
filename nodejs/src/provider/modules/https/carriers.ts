import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const httpsServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpsServer" };

export const httpsClientRequestCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpsClientRequest" };
