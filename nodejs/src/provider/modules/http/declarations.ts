import {
  rustCallableTargetType,
  rustJsArrayTargetType,
  rustOptionTargetType,
} from "@tsonic/target-rust/provider";
import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustTargetTypeRef,
} from "@tsonic/target-rust/provider";
import { providerCallbackType, providerRef } from "../../declarations/builders.js";
import {
  boolCarrier,
  emptyCallbackCarrier,
  int32Carrier,
  nodeErrorCarrier,
  stringCarrier,
  unitCarrier,
} from "../../model/carriers.js";
import { providerNativeFallibility } from "../../model/operations.js";
import {
  booleanType,
  int32Type,
  stringType,
  undefinedType,
  voidType,
} from "../../model/source-types.js";
import type { ProviderTypeExpr } from "../../model/source-types.js";
import { bufferCarrier } from "../buffer/carriers.js";
import { netSocketCarrier } from "../net/carriers.js";
import {
  httpAddressInfoCarrier,
  httpCloseCallbackCarrier,
  httpIncomingMessageCarrier,
  httpRequestCallbackCarrier,
  httpServerAddressCarrier,
  httpServerCarrier,
  httpServerResponseCarrier,
  incomingHttpHeadersCarrier,
  outgoingHttpHeadersCarrier,
} from "./carriers.js";

const moduleSpecifier = "node:http";
const incomingHeadersId = `${moduleSpecifier}::IncomingHttpHeaders`;
const incomingHeaderValuesId = `${moduleSpecifier}::IncomingHttpHeaderValues`;
const outgoingHeadersId = `${moduleSpecifier}::OutgoingHttpHeaders`;
const addressInfoId = `${moduleSpecifier}::AddressInfo`;
const serverAddressId = `${moduleSpecifier}::ServerAddress`;
const incomingId = `${moduleSpecifier}::IncomingMessage`;
const responseId = `${moduleSpecifier}::ServerResponse`;
const serverId = `${moduleSpecifier}::Server`;
const bufferType = providerRef("node:buffer", "Buffer");
const errorType = { kind: "source-global", name: "Error" } as const;
const optionalStringType = optional(stringType);
const optionalInt32Type = optional(int32Type);
const stringArrayType = { kind: "array", elementType: stringType } as const;
const stringArrayCarrier = rustJsArrayTargetType(stringCarrier);
const emptyListenerCarrier = rustCallableTargetType([], unitCarrier);
const errorListenerCarrier = rustCallableTargetType([nodeErrorCarrier], unitCarrier);
const dataListenerCarrier = rustCallableTargetType([bufferCarrier], unitCarrier);
const optionalErrorCarrier = rustOptionTargetType(nodeErrorCarrier);
const optionalStringCarrier = rustOptionTargetType(stringCarrier);
const optionalInt32Carrier = rustOptionTargetType(int32Carrier);
const optionalAddressCarrier = rustOptionTargetType(httpAddressInfoCarrier);
const socketReferenceCarrier: RustTargetTypeRef = {
  kind: "reference",
  referent: netSocketCarrier,
  mutable: false,
};

export function httpModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.http",
    imports: [
      { moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] },
      { moduleSpecifier: "node:net", namedImports: [{ exportedName: "Socket" }] },
      { moduleSpecifier: "node:stream", namedImports: [{ exportedName: "Readable" }, { exportedName: "Writable" }] },
    ],
    exports: [
      headerDeclaration(incomingHeadersId, "IncomingHttpHeaders"),
      {
        id: incomingHeaderValuesId,
        name: "IncomingHttpHeaderValues",
        kind: "class",
        members: [{
          id: `${incomingHeaderValuesId}.indexer`,
          name: "indexer",
          kind: "indexer",
          signatures: [{
            id: `${incomingHeaderValuesId}.indexer(name)`,
            parameters: [{ name: "name", type: stringType }],
            returnType: optional(stringArrayType),
          }],
        }],
      },
      headerDeclaration(outgoingHeadersId, "OutgoingHttpHeaders"),
      {
        id: addressInfoId,
        name: "AddressInfo",
        kind: "class",
        members: [
          property(addressInfoId, "address", stringType),
          property(addressInfoId, "family", stringType),
          property(addressInfoId, "port", int32Type),
        ],
      },
      {
        id: serverAddressId,
        name: "ServerAddress",
        kind: "class",
        members: [
          property(serverAddressId, "address", optional(providerRef(moduleSpecifier, "AddressInfo"))),
          property(serverAddressId, "path", optionalStringType),
          property(serverAddressId, "port", optionalInt32Type),
        ],
      },
      incomingMessageDeclaration(),
      serverResponseDeclaration(),
      serverDeclaration(),
      {
        id: `${moduleSpecifier}::createServer`,
        name: "createServer",
        kind: "function",
        signatures: [{
          id: `${moduleSpecifier}::createServer(listener)`,
          parameters: [{
            name: "listener",
            type: callbackType(`${moduleSpecifier}::createServer(listener)`, [
              { name: "request", type: providerRef(moduleSpecifier, "IncomingMessage") },
              { name: "response", type: providerRef(moduleSpecifier, "ServerResponse") },
            ]),
            optional: true,
          }],
          returnType: providerRef(moduleSpecifier, "Server"),
        }],
      },
    ],
  };
}

function headerDeclaration(id: string, name: string): RustProviderModuleDefinition["exports"][number] {
  return {
    id,
    name,
    kind: "class",
    members: [
      method(id, "get", [{ name: "name", type: stringType }], optionalStringType),
      method(id, "getAll", [{ name: "name", type: stringType }], stringArrayType),
      method(id, "names", [], stringArrayType),
    ],
  };
}

function incomingMessageDeclaration(): RustProviderModuleDefinition["exports"][number] {
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
      method(incomingId, "destroy", [{ name: "error", type: errorType, optional: true }], classType),
      ...eventMembers(incomingId, classType, [
        ["data", [{ name: "chunk", type: bufferType }]],
        ["end", []],
        ["aborted", []],
        ["error", [{ name: "error", type: errorType }]],
        ["close", []],
      ]),
    ],
  };
}

function serverResponseDeclaration(): RustProviderModuleDefinition["exports"][number] {
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
      method(responseId, "destroy", [{ name: "error", type: errorType, optional: true }], classType),
      ...eventMembers(responseId, classType, [
        ["drain", []],
        ["finish", []],
        ["error", [{ name: "error", type: errorType }]],
        ["close", []],
      ]),
    ],
  };
}

function serverDeclaration(): RustProviderModuleDefinition["exports"][number] {
  const classType = providerRef(moduleSpecifier, "Server");
  return {
    id: serverId,
    name: "Server",
    kind: "class",
    members: [
      property(serverId, "listening", booleanType),
      {
        id: `${serverId}.listen`,
        name: "listen",
        kind: "method",
        signatures: [
          {
            id: `${serverId}.listen(port,callback)`,
            parameters: [
              { name: "port", type: int32Type },
              { name: "callback", type: callbackType(`${serverId}.listen(port,callback)`, []), optional: true },
            ],
            returnType: classType,
          },
          {
            id: `${serverId}.listen(port,host,callback)`,
            parameters: [
              { name: "port", type: int32Type },
              { name: "hostname", type: stringType },
              { name: "callback", type: callbackType(`${serverId}.listen(port,host,callback)`, []), optional: true },
            ],
            returnType: classType,
          },
          {
            id: `${serverId}.listen(port,host,backlog,callback)`,
            parameters: [
              { name: "port", type: int32Type },
              { name: "hostname", type: stringType },
              { name: "backlog", type: int32Type },
              { name: "callback", type: callbackType(`${serverId}.listen(port,host,backlog,callback)`, []), optional: true },
            ],
            returnType: classType,
          },
          {
            id: `${serverId}.listen(path,callback)`,
            parameters: [
              { name: "path", type: stringType },
              { name: "callback", type: callbackType(`${serverId}.listen(path,callback)`, []), optional: true },
            ],
            returnType: classType,
          },
        ],
      },
      method(serverId, "address", [], optional(providerRef(moduleSpecifier, "ServerAddress"))),
      method(serverId, "close", [{
        name: "callback",
        type: callbackType(`${serverId}.close.callback`, [{ name: "error", type: errorType, optional: true }]),
        optional: true,
      }], classType),
      method(serverId, "ref", [], classType),
      method(serverId, "unref", [], classType),
      ...eventMembers(serverId, classType, [
        ["error", [{ name: "error", type: errorType }]],
        ["listening", []],
        ["close", []],
      ]),
    ],
  };
}

export function httpRows(): readonly RustProviderOperationDefinition[] {
  return [
    {
      exportId: `${moduleSpecifier}::createServer`,
      signatureId: `${moduleSpecifier}::createServer(listener)`,
      operationKind: "method",
      target: { form: "call", path: "node_http::create_server_optional", argModes: ["value"] },
      resultCarrier: httpServerCarrier,
      parameterCarriers: [rustOptionTargetType(httpRequestCallbackCarrier)],
    },
    ...headerRows(incomingHeadersId, incomingHttpHeadersCarrier),
    {
      exportId: incomingHeaderValuesId,
      memberId: `${incomingHeaderValuesId}.indexer`,
      operationKind: "indexer",
      target: { form: "receiver-method", name: "get_values", argModes: ["ref"] },
      receiverCarrier: incomingHttpHeadersCarrier,
      resultCarrier: rustOptionTargetType(stringArrayCarrier),
      parameterCarriers: [stringCarrier],
      ...providerNativeFallibility,
    },
    ...headerRows(outgoingHeadersId, outgoingHttpHeadersCarrier),
    ...addressRows(),
    ...incomingRows(),
    ...responseRows(),
    ...serverRows(),
  ];
}

function headerRows(
  exportId: string,
  receiverCarrier: RustTargetTypeRef,
): readonly RustProviderOperationDefinition[] {
  return [
    fallibleReceiver(exportId, "get", "get", receiverCarrier, optionalStringCarrier, [stringCarrier], undefined, ["ref"]),
    fallibleReceiver(exportId, "getAll", "get_all", receiverCarrier, stringArrayCarrier, [stringCarrier], undefined, ["ref"]),
    receiver(exportId, "names", "names", receiverCarrier, stringArrayCarrier, []),
  ];
}

function addressRows(): readonly RustProviderOperationDefinition[] {
  return [
    field(addressInfoId, "address", httpAddressInfoCarrier, stringCarrier),
    field(addressInfoId, "family", httpAddressInfoCarrier, stringCarrier),
    field(addressInfoId, "port", httpAddressInfoCarrier, int32Carrier),
    receiver(serverAddressId, "address", "address", httpServerAddressCarrier, optionalAddressCarrier, []),
    receiver(serverAddressId, "path", "path", httpServerAddressCarrier, optionalStringCarrier, []),
    receiver(serverAddressId, "port", "port", httpServerAddressCarrier, optionalInt32Carrier, []),
  ];
}

function incomingRows(): readonly RustProviderOperationDefinition[] {
  const rows: RustProviderOperationDefinition[] = [
    receiver(incomingId, "method", "method", httpIncomingMessageCarrier, optionalStringCarrier, [], "property"),
    receiver(incomingId, "url", "url", httpIncomingMessageCarrier, optionalStringCarrier, [], "property"),
    receiver(incomingId, "httpVersion", "http_version", httpIncomingMessageCarrier, stringCarrier, [], "property"),
    receiver(incomingId, "headers", "headers", httpIncomingMessageCarrier, incomingHttpHeadersCarrier, [], "property"),
    receiver(incomingId, "headersDistinct", "headers_distinct", httpIncomingMessageCarrier, incomingHttpHeadersCarrier, [], "property"),
    receiver(incomingId, "complete", "complete", httpIncomingMessageCarrier, boolCarrier, [], "property"),
    receiver(incomingId, "aborted", "aborted", httpIncomingMessageCarrier, boolCarrier, [], "property"),
    receiver(incomingId, "destroyed", "destroyed", httpIncomingMessageCarrier, boolCarrier, [], "property"),
    receiver(incomingId, "statusCode", "status_code", httpIncomingMessageCarrier, optionalInt32Carrier, [], "property"),
    receiver(incomingId, "statusMessage", "status_message", httpIncomingMessageCarrier, optionalStringCarrier, [], "property"),
    receiver(incomingId, "socket", "socket", httpIncomingMessageCarrier, socketReferenceCarrier, [], "property"),
    fallibleReceiver(incomingId, "destroy", "destroy_chain", httpIncomingMessageCarrier, httpIncomingMessageCarrier, [optionalErrorCarrier]),
  ];
  rows.push(...eventRows(incomingId, httpIncomingMessageCarrier, [
    ["data", dataListenerCarrier],
    ["end", emptyListenerCarrier],
    ["aborted", emptyListenerCarrier],
    ["error", errorListenerCarrier],
    ["close", emptyListenerCarrier],
  ]));
  return rows;
}

function responseRows(): readonly RustProviderOperationDefinition[] {
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
    ["error", errorListenerCarrier],
    ["close", emptyListenerCarrier],
  ]));
  return rows;
}

function serverRows(): readonly RustProviderOperationDefinition[] {
  const optionalCallback = rustOptionTargetType(emptyCallbackCarrier);
  const rows: RustProviderOperationDefinition[] = [
    receiver(serverId, "listening", "listening", httpServerCarrier, boolCarrier, [], "property"),
    fallibleReceiver(serverId, "listen", "listen_default_host_optional", httpServerCarrier, httpServerCarrier, [int32Carrier, optionalCallback], `${serverId}.listen(port,callback)`, ["value", "value"]),
    fallibleReceiver(serverId, "listen", "listen_optional", httpServerCarrier, httpServerCarrier, [int32Carrier, stringCarrier, optionalCallback], `${serverId}.listen(port,host,callback)`, ["value", "ref", "value"]),
    fallibleReceiver(serverId, "listen", "listen_with_backlog_optional", httpServerCarrier, httpServerCarrier, [int32Carrier, stringCarrier, int32Carrier, optionalCallback], `${serverId}.listen(port,host,backlog,callback)`, ["value", "ref", "value", "value"]),
    fallibleReceiver(serverId, "listen", "listen_path_optional", httpServerCarrier, httpServerCarrier, [stringCarrier, optionalCallback], `${serverId}.listen(path,callback)`, ["ref", "value"]),
    receiver(serverId, "address", "address", httpServerCarrier, rustOptionTargetType(httpServerAddressCarrier), []),
    fallibleReceiver(serverId, "close", "close_optional", httpServerCarrier, httpServerCarrier, [rustOptionTargetType(httpCloseCallbackCarrier)]),
    receiver(serverId, "ref", "ref_chain", httpServerCarrier, httpServerCarrier, []),
    receiver(serverId, "unref", "unref_chain", httpServerCarrier, httpServerCarrier, []),
  ];
  rows.push(...eventRows(serverId, httpServerCarrier, [
    ["error", errorListenerCarrier],
    ["listening", emptyListenerCarrier],
    ["close", emptyListenerCarrier],
  ]));
  return rows;
}

function eventRows(
  exportId: string,
  receiverCarrier: RustTargetTypeRef,
  events: readonly (readonly [name: string, listener: RustTargetTypeRef])[],
): readonly RustProviderOperationDefinition[] {
  return (["on", "once", "off"] as const).flatMap(methodName => events.map(([eventName, listener]) => ({
    exportId,
    memberId: `${exportId}.${methodName}`,
    signatureId: `${exportId}.${methodName}.${eventName}`,
    operationKind: "method" as const,
    target: {
      form: "receiver-method" as const,
      name: `${methodName}_${eventName}`,
      argModes: ["ref", "ref"] as const,
    },
    receiverCarrier,
    resultCarrier: receiverCarrier,
    parameterCarriers: [stringCarrier, listener],
    ...providerNativeFallibility,
  })));
}

function receiver(
  exportId: string,
  memberName: string,
  targetName: string,
  receiverCarrier: RustTargetTypeRef,
  resultCarrier: RustTargetTypeRef,
  parameterCarriers: readonly RustTargetTypeRef[],
  operationKind: "method" | "property" = "method",
): RustProviderOperationDefinition {
  return {
    exportId,
    memberId: `${exportId}.${memberName}`,
    operationKind,
    target: { form: "receiver-method", name: targetName },
    receiverCarrier,
    resultCarrier,
    parameterCarriers,
  };
}

function fallibleReceiver(
  exportId: string,
  memberName: string,
  targetName: string,
  receiverCarrier: RustTargetTypeRef,
  resultCarrier: RustTargetTypeRef,
  parameterCarriers: readonly RustTargetTypeRef[],
  signatureId?: string,
  argModes?: readonly ("value" | "ref" | "mut-ref")[],
  operationKind: "method" | "property-set" = "method",
): RustProviderOperationDefinition {
  return {
    exportId,
    memberId: `${exportId}.${memberName}`,
    ...(signatureId === undefined ? {} : { signatureId }),
    operationKind,
    target: {
      form: "receiver-method",
      name: targetName,
      ...(argModes === undefined ? {} : { argModes }),
    },
    receiverCarrier,
    resultCarrier,
    parameterCarriers,
    ...providerNativeFallibility,
  };
}

function field(
  exportId: string,
  memberName: string,
  receiverCarrier: RustTargetTypeRef,
  resultCarrier: RustTargetTypeRef,
): RustProviderOperationDefinition {
  return {
    exportId,
    memberId: `${exportId}.${memberName}`,
    operationKind: "property",
    target: { form: "field", name: memberName },
    receiverCarrier,
    resultCarrier,
  };
}

function method(
  classId: string,
  name: string,
  parameters: readonly {
    readonly name: string;
    readonly type: ProviderTypeExpr;
    readonly optional?: boolean;
  }[],
  returnType: ProviderTypeExpr,
) {
  return {
    id: `${classId}.${name}`,
    name,
    kind: "method" as const,
    signatures: [{
      id: `${classId}.${name}(${parameters.map(parameter => parameter.name).join(",")})`,
      parameters,
      returnType,
    }],
  };
}

function property(
  classId: string,
  name: string,
  type: ProviderTypeExpr,
  readonly = true,
) {
  return {
    id: `${classId}.${name}`,
    name,
    kind: "property" as const,
    ...(readonly ? { readonly: true } : {}),
    type,
  };
}

function eventMembers(
  classId: string,
  returnType: ProviderTypeExpr,
  events: readonly (readonly [
    name: string,
    parameters: readonly {
      readonly name: string;
      readonly type: ProviderTypeExpr;
    }[],
  ])[],
) {
  return (["on", "once", "off"] as const).map(methodName => ({
    id: `${classId}.${methodName}`,
    name: methodName,
    kind: "method" as const,
    signatures: events.map(([eventName, parameters]) => ({
      id: `${classId}.${methodName}.${eventName}`,
      parameters: [
        { name: "event", type: { kind: "literal" as const, value: eventName } },
        { name: "listener", type: providerCallbackType(`${classId}.${methodName}.${eventName}`, "listener", parameters) },
      ],
      returnType,
    })),
  }));
}

function callbackType(
  signatureId: string,
  parameters: readonly {
    readonly name: string;
    readonly type: ProviderTypeExpr;
    readonly optional?: boolean;
  }[],
): ProviderTypeExpr {
  return {
    kind: "function",
    id: `${signatureId}::callback`,
    parameters,
    returnType: voidType,
  };
}

function optional(type: ProviderTypeExpr): ProviderTypeExpr {
  return { kind: "union", types: [type, undefinedType] };
}
