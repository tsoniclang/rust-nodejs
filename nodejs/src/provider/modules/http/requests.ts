import { rustBorrowedStrTargetType, rustOptionTargetType } from "@tsonic/target-rust/provider";
import type { RustProviderModuleDefinition, RustProviderOperationDefinition, RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { providerRef } from "../../declarations/builders.js";
import { boolCarrier } from "../../model/carriers.js";
import { booleanType, errorType as retainedErrorType, stringType } from "../../model/source-types.js";
import { httpIncomingMessageCarrier, incomingHttpHeadersCarrier } from "./carriers.js";
import {
  moduleSpecifier,
  incomingId,
  bufferType,
  optionalStringType,
  optionalInt32Type,
  emptyListenerCarrier,
  retainedErrorListenerCarrier,
  dataListenerCarrier,
  optionalErrorCarrier,
  optionalInt32Carrier,
  socketReferenceCarrier,
} from "./types.js";
import {
  eventRows,
  receiver,
  fallibleReceiver,
  method,
  property,
  eventMembers,
} from "./members.js";

export function incomingMessageDeclaration(): RustProviderModuleDefinition["exports"][number] {
  const classType = providerRef(moduleSpecifier, "IncomingMessage");
  return {
    id: incomingId,
    name: "IncomingMessage",
    kind: "class",
    heritage: [{ kind: "extends", type: providerRef("node:stream", "Readable") }],
    members: [
      property(incomingId, "method", optionalStringType),
      property(incomingId, "url", optionalStringType),
      property(incomingId, "httpVersion", stringType),
      property(incomingId, "headers", providerRef(moduleSpecifier, "IncomingHttpHeaders")),
      property(incomingId, "headersDistinct", providerRef(moduleSpecifier, "IncomingHttpHeaderValues")),
      property(incomingId, "complete", booleanType),
      property(incomingId, "aborted", booleanType),
      property(incomingId, "destroyed", booleanType),
      property(incomingId, "statusCode", optionalInt32Type),
      property(incomingId, "statusMessage", optionalStringType),
      property(incomingId, "socket", providerRef("node:net", "Socket")),
      method(incomingId, "destroy", [{ name: "error", type: retainedErrorType, optional: true }], classType),
      ...eventMembers(incomingId, classType, [
        ["data", [{ name: "chunk", type: bufferType }]],
        ["end", []],
        ["aborted", []],
        ["error", [{ name: "error", type: retainedErrorType }]],
        ["close", []],
      ]),
    ],
  };
}

export function incomingRows(): readonly RustProviderOperationDefinition[] {
  const borrowedString = rustBorrowedStrTargetType();
  const optionalBorrowedString = rustOptionTargetType(borrowedString);
  const borrowedHeaders: RustTargetTypeRef = { kind: "reference", mutable: false, referent: incomingHttpHeadersCarrier };
  const rows: RustProviderOperationDefinition[] = [
    receiver(incomingId, "method", "method", httpIncomingMessageCarrier, optionalBorrowedString, [], "property"),
    receiver(incomingId, "url", "url", httpIncomingMessageCarrier, optionalBorrowedString, [], "property"),
    receiver(incomingId, "httpVersion", "http_version", httpIncomingMessageCarrier, borrowedString, [], "property"),
    receiver(incomingId, "headers", "headers", httpIncomingMessageCarrier, borrowedHeaders, [], "property"),
    receiver(incomingId, "headersDistinct", "headers_distinct", httpIncomingMessageCarrier, borrowedHeaders, [], "property"),
    receiver(incomingId, "complete", "complete", httpIncomingMessageCarrier, boolCarrier, [], "property"),
    receiver(incomingId, "aborted", "aborted", httpIncomingMessageCarrier, boolCarrier, [], "property"),
    receiver(incomingId, "destroyed", "destroyed", httpIncomingMessageCarrier, boolCarrier, [], "property"),
    receiver(incomingId, "statusCode", "status_code", httpIncomingMessageCarrier, optionalInt32Carrier, [], "property"),
    receiver(incomingId, "statusMessage", "status_message", httpIncomingMessageCarrier, optionalBorrowedString, [], "property"),
    receiver(incomingId, "socket", "socket", httpIncomingMessageCarrier, socketReferenceCarrier, [], "property"),
    fallibleReceiver(incomingId, "destroy", "destroy_chain", httpIncomingMessageCarrier, httpIncomingMessageCarrier, [optionalErrorCarrier]),
  ];
  rows.push(...eventRows(incomingId, httpIncomingMessageCarrier, [
    ["data", dataListenerCarrier],
    ["end", emptyListenerCarrier],
    ["aborted", emptyListenerCarrier],
    ["error", retainedErrorListenerCarrier],
    ["close", emptyListenerCarrier],
  ]));
  return rows;
}
