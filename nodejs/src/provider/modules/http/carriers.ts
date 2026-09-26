import { rustCallableTargetType, rustNamedTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { nodeErrorCarrier, unitCarrier } from "../../model/carriers.js";
import { readableCarrier, streamCarrier, writableCarrier } from "../stream/carriers.js";
import { rustOptionTargetType } from "@tsonic/target-rust/provider";

export const incomingHttpHeadersCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.IncomingHttpHeaders",
  "tsonic_rust_node::http::IncomingHttpHeaders",
);

export const outgoingHttpHeadersCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.OutgoingHttpHeaders",
  "tsonic_rust_node::http::OutgoingHttpHeaders",
);

export const httpAddressInfoCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.HttpAddressInfo",
  "tsonic_rust_node::http::AddressInfo",
);

export const httpServerAddressCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.HttpServerAddress",
  "tsonic_rust_node::http::ServerAddress",
);

export const httpIncomingMessageCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.HttpIncomingMessage",
  "tsonic_rust_node::http::IncomingMessage",
  [],
  [],
  undefined,
  [
    { target: streamCarrier, path: "tsonic_rust_node::http::incoming_message_as_stream" },
    { target: readableCarrier, path: "tsonic_rust_node::http::incoming_message_as_readable" },
  ],
);

export const httpServerResponseCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.HttpServerResponse",
  "tsonic_rust_node::http::ServerResponse",
  [],
  [],
  undefined,
  [
    { target: streamCarrier, path: "tsonic_rust_node::http::server_response_as_stream" },
    { target: writableCarrier, path: "tsonic_rust_node::http::server_response_as_writable" },
  ],
);

export const httpServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpServer" };

export const httpRequestCallbackCarrier = rustCallableTargetType(
  [httpIncomingMessageCarrier, httpServerResponseCarrier],
  unitCarrier,
);

export const httpResponseCallbackCarrier = rustCallableTargetType(
  [httpIncomingMessageCarrier],
  unitCarrier,
);

export const httpCloseCallbackCarrier = rustCallableTargetType(
  [rustOptionTargetType(nodeErrorCarrier)],
  unitCarrier,
);
