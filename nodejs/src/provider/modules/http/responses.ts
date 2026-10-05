import { rustOptionTargetType } from "@tsonic/target-rust/provider";
import type { RustProviderModuleDefinition, RustProviderOperationDefinition } from "@tsonic/target-rust/provider";
import { providerRef } from "../../declarations/builders.js";
import {
  boolCarrier,
  int32Carrier,
  stringCarrier,
  unitCarrier,
} from "../../model/carriers.js";
import {
  booleanType,
  errorType as retainedErrorType,
  int32Type,
  stringType,
  voidType,
} from "../../model/source-types.js";
import { bufferCarrier } from "../buffer/carriers.js";
import { httpServerResponseCarrier, outgoingHttpHeadersCarrier } from "./carriers.js";
import {
  moduleSpecifier,
  responseId,
  bufferType,
  optionalStringType,
  stringArrayType,
  stringArrayCarrier,
  emptyListenerCarrier,
  retainedErrorListenerCarrier,
  optionalErrorCarrier,
  optionalStringCarrier,
} from "./types.js";
import {
  eventRows,
  receiver,
  fallibleReceiver,
  method,
  property,
  eventMembers,
} from "./members.js";

export function serverResponseDeclaration(): RustProviderModuleDefinition["exports"][number] {
  const classType = providerRef(moduleSpecifier, "ServerResponse");
  const headerValueSignatures = (name: "setHeader" | "appendHeader") => ({
    id: `${responseId}.${name}`,
    name,
    kind: "method" as const,
    signatures: [
      {
        id: `${responseId}.${name}(string)`,
        parameters: [{ name: "name", type: stringType }, { name: "value", type: stringType }],
        returnType: classType,
      },
      {
        id: `${responseId}.${name}(strings)`,
        parameters: [{ name: "name", type: stringType }, { name: "value", type: stringArrayType }],
        returnType: classType,
      },
    ],
  });
  return {
    id: responseId,
    name: "ServerResponse",
    kind: "class",
    heritage: [{ kind: "extends", type: providerRef("node:stream", "Writable") }],
    members: [
      property(responseId, "statusCode", int32Type, false),
      property(responseId, "statusMessage", stringType, false),
      property(responseId, "headersSent", booleanType),
      property(responseId, "writableEnded", booleanType),
      property(responseId, "writableFinished", booleanType),
      property(responseId, "writableNeedDrain", booleanType),
      property(responseId, "destroyed", booleanType),
      headerValueSignatures("setHeader"),
      headerValueSignatures("appendHeader"),
      method(responseId, "getHeader", [{ name: "name", type: stringType }], optionalStringType),
      method(responseId, "getHeaderValues", [{ name: "name", type: stringType }], stringArrayType),
      method(responseId, "getHeaderNames", [], stringArrayType),
      method(responseId, "getHeaders", [], providerRef(moduleSpecifier, "OutgoingHttpHeaders")),
      method(responseId, "hasHeader", [{ name: "name", type: stringType }], booleanType),
      method(responseId, "removeHeader", [{ name: "name", type: stringType }], voidType),
      method(responseId, "flushHeaders", [], voidType),
      {
        id: `${responseId}.writeHead`,
        name: "writeHead",
        kind: "method",
        signatures: [
          {
            id: `${responseId}.writeHead(code)`,
            parameters: [{ name: "statusCode", type: int32Type }],
            returnType: classType,
          },
          {
            id: `${responseId}.writeHead(code,headers)`,
            parameters: [
              { name: "statusCode", type: int32Type },
              { name: "headers", type: providerRef(moduleSpecifier, "OutgoingHttpHeaders") },
            ],
            returnType: classType,
          },
          {
            id: `${responseId}.writeHead(code,message,headers)`,
            parameters: [
              { name: "statusCode", type: int32Type },
              { name: "statusMessage", type: stringType },
              { name: "headers", type: providerRef(moduleSpecifier, "OutgoingHttpHeaders"), optional: true },
            ],
            returnType: classType,
          },
        ],
      },
      {
        id: `${responseId}.write`,
        name: "write",
        kind: "method",
        signatures: [
          { id: `${responseId}.write(string)`, parameters: [{ name: "chunk", type: stringType }], returnType: booleanType },
          { id: `${responseId}.write(buffer)`, parameters: [{ name: "chunk", type: bufferType }], returnType: booleanType },
        ],
      },
      {
        id: `${responseId}.end`,
        name: "end",
        kind: "method",
        signatures: [
          { id: `${responseId}.end()`, parameters: [], returnType: classType },
          { id: `${responseId}.end(string)`, parameters: [{ name: "chunk", type: stringType }], returnType: classType },
          { id: `${responseId}.end(buffer)`, parameters: [{ name: "chunk", type: bufferType }], returnType: classType },
        ],
      },
      method(responseId, "destroy", [{ name: "error", type: retainedErrorType, optional: true }], classType),
      ...eventMembers(responseId, classType, [
        ["drain", []],
        ["finish", []],
        ["error", [{ name: "error", type: retainedErrorType }]],
        ["close", []],
      ]),
    ],
  };
}

export function responseRows(): readonly RustProviderOperationDefinition[] {
  const rows: RustProviderOperationDefinition[] = [
    receiver(responseId, "statusCode", "status_code", httpServerResponseCarrier, int32Carrier, [], "property"),
    fallibleReceiver(responseId, "statusCode", "set_status_code", httpServerResponseCarrier, unitCarrier, [int32Carrier], undefined, ["value"], "property-set"),
    receiver(responseId, "statusMessage", "status_message", httpServerResponseCarrier, stringCarrier, [], "property"),
    fallibleReceiver(responseId, "statusMessage", "set_status_message", httpServerResponseCarrier, unitCarrier, [stringCarrier], undefined, ["ref"], "property-set"),
    receiver(responseId, "headersSent", "headers_sent", httpServerResponseCarrier, boolCarrier, [], "property"),
    receiver(responseId, "writableEnded", "writable_ended", httpServerResponseCarrier, boolCarrier, [], "property"),
    receiver(responseId, "writableFinished", "writable_finished", httpServerResponseCarrier, boolCarrier, [], "property"),
    receiver(responseId, "writableNeedDrain", "writable_need_drain", httpServerResponseCarrier, boolCarrier, [], "property"),
    receiver(responseId, "destroyed", "destroyed", httpServerResponseCarrier, boolCarrier, [], "property"),
    fallibleReceiver(responseId, "setHeader", "set_header", httpServerResponseCarrier, httpServerResponseCarrier, [stringCarrier, stringCarrier], `${responseId}.setHeader(string)`, ["ref", "ref"]),
    fallibleReceiver(responseId, "setHeader", "set_header_values", httpServerResponseCarrier, httpServerResponseCarrier, [stringCarrier, stringArrayCarrier], `${responseId}.setHeader(strings)`, ["ref", "ref"]),
    fallibleReceiver(responseId, "appendHeader", "append_header", httpServerResponseCarrier, httpServerResponseCarrier, [stringCarrier, stringCarrier], `${responseId}.appendHeader(string)`, ["ref", "ref"]),
    fallibleReceiver(responseId, "appendHeader", "append_header_values", httpServerResponseCarrier, httpServerResponseCarrier, [stringCarrier, stringArrayCarrier], `${responseId}.appendHeader(strings)`, ["ref", "ref"]),
    fallibleReceiver(responseId, "getHeader", "get_header", httpServerResponseCarrier, optionalStringCarrier, [stringCarrier], undefined, ["ref"]),
    fallibleReceiver(responseId, "getHeaderValues", "get_header_values", httpServerResponseCarrier, stringArrayCarrier, [stringCarrier], undefined, ["ref"]),
    receiver(responseId, "getHeaderNames", "get_header_names", httpServerResponseCarrier, stringArrayCarrier, []),
    receiver(responseId, "getHeaders", "get_headers", httpServerResponseCarrier, outgoingHttpHeadersCarrier, []),
    fallibleReceiver(responseId, "hasHeader", "has_header", httpServerResponseCarrier, boolCarrier, [stringCarrier], undefined, ["ref"]),
    fallibleReceiver(responseId, "removeHeader", "remove_header", httpServerResponseCarrier, unitCarrier, [stringCarrier], undefined, ["ref"]),
    fallibleReceiver(responseId, "flushHeaders", "flush_headers", httpServerResponseCarrier, unitCarrier, []),
    fallibleReceiver(responseId, "writeHead", "write_head", httpServerResponseCarrier, httpServerResponseCarrier, [int32Carrier], `${responseId}.writeHead(code)`, ["value"]),
    fallibleReceiver(responseId, "writeHead", "write_head_headers", httpServerResponseCarrier, httpServerResponseCarrier, [int32Carrier, outgoingHttpHeadersCarrier], `${responseId}.writeHead(code,headers)`, ["value", "ref"]),
    fallibleReceiver(responseId, "writeHead", "write_head_message", httpServerResponseCarrier, httpServerResponseCarrier, [int32Carrier, stringCarrier, rustOptionTargetType(outgoingHttpHeadersCarrier)], `${responseId}.writeHead(code,message,headers)`, ["value", "ref", "ref"]),
    fallibleReceiver(responseId, "write", "write_string", httpServerResponseCarrier, boolCarrier, [stringCarrier], `${responseId}.write(string)`, ["ref"]),
    fallibleReceiver(responseId, "write", "write_buffer", httpServerResponseCarrier, boolCarrier, [bufferCarrier], `${responseId}.write(buffer)`, ["ref"]),
    fallibleReceiver(responseId, "end", "end_empty", httpServerResponseCarrier, httpServerResponseCarrier, [], `${responseId}.end()`),
    fallibleReceiver(responseId, "end", "end_string", httpServerResponseCarrier, httpServerResponseCarrier, [stringCarrier], `${responseId}.end(string)`, ["ref"]),
    fallibleReceiver(responseId, "end", "end_buffer", httpServerResponseCarrier, httpServerResponseCarrier, [bufferCarrier], `${responseId}.end(buffer)`, ["ref"]),
    fallibleReceiver(responseId, "destroy", "destroy_chain", httpServerResponseCarrier, httpServerResponseCarrier, [optionalErrorCarrier]),
  ];
  rows.push(...eventRows(responseId, httpServerResponseCarrier, [
    ["drain", emptyListenerCarrier],
    ["finish", emptyListenerCarrier],
    ["error", retainedErrorListenerCarrier],
    ["close", emptyListenerCarrier],
  ]));
  return rows;
}
