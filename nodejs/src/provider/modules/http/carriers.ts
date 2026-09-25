import { rustCallableTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { unitCarrier } from "../../model/carriers.js";

export const httpIncomingMessageCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpIncomingMessage" };

export const httpServerResponseCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpServerResponse" };

export const httpServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpServer" };

export const httpRequestCallbackCarrier = rustCallableTargetType(
  [httpIncomingMessageCarrier, httpServerResponseCarrier],
  unitCarrier,
);

export const httpResponseCallbackCarrier = rustCallableTargetType(
  [httpIncomingMessageCarrier],
  unitCarrier,
);
