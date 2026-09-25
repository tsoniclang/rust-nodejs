import { rustCallableTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { unitCarrier } from "../../model/carriers.js";

export const tlsConnectOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.TlsConnectOptions" };

export const tlsServerOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.TlsServerOptions" };

export const tlsSocketCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.TlsSocket" };

export const tlsServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.TlsServer" };

export const tlsSocketCallbackCarrier = rustCallableTargetType(
  [tlsSocketCarrier],
  unitCarrier,
);
