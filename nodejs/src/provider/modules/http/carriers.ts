import { rustCallableTargetType, rustNamedTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { nodeErrorCarrier, unitCarrier, nodeProgramErrorArguments } from "../../model/carriers.js";
import { readableCarrier, streamCarrier, writableCarrier } from "../stream/carriers.js";
import { rustOptionTargetType } from "@tsonic/target-rust/provider";

export const incomingHttpHeadersCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.IncomingHttpHeaders",
  "tsonic_rust_node::http::IncomingHttpHeaders",
  [], [], undefined, [], undefined, [],
);

export const outgoingHttpHeadersCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.OutgoingHttpHeaders",
  "tsonic_rust_node::http::OutgoingHttpHeaders",
  [], [], undefined, [], undefined, [],
);

export const httpAddressInfoCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.HttpAddressInfo",
  "tsonic_rust_node::http::AddressInfo",
  [], [], undefined, [], undefined, [],
);

export const httpServerAddressCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.HttpServerAddress",
  "tsonic_rust_node::http::ServerAddress",
  [], [], undefined, [], undefined, [],
);

export const httpIncomingMessageCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.HttpIncomingMessage",
  "tsonic_rust_node::http::IncomingMessage",
  nodeProgramErrorArguments,
  [],
  undefined,
  [
    { target: streamCarrier, path: "tsonic_rust_node::http::incoming_message_as_stream" },
    { target: readableCarrier, path: "tsonic_rust_node::http::incoming_message_as_readable" },
  ],
  undefined,
  [],
);

export const httpServerResponseCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.HttpServerResponse",
  "tsonic_rust_node::http::ServerResponse",
  nodeProgramErrorArguments,
  [],
  undefined,
  [
    { target: streamCarrier, path: "tsonic_rust_node::http::server_response_as_stream" },
    { target: writableCarrier, path: "tsonic_rust_node::http::server_response_as_writable" },
  ],
  undefined,
  [],
);

export const httpServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpServer", genericArguments: nodeProgramErrorArguments };

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
