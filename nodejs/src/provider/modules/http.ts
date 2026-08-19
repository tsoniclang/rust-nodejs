import {
  bufferCarrier,
  emptyCallbackCarrier,
  fnExport,
  httpRequestCallbackCarrier,
  httpServerCarrier,
  int32Carrier,
  int32Type,
  methodMember,
  propertyMember,
  providerNativeFallibility,
  providerRef,
  stringCarrier,
  stringType,
  unitCarrier,
  voidType,
} from "../model.js";

import type {
  ProviderTypeExpr,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";
// --- node:http ---------------------------------------------------------------

export function httpModule(): RustProviderModuleDefinition {
  const m = "node:http";
  const incomingId = `${m}::IncomingMessage`;
  const responseId = `${m}::ServerResponse`;
  const serverId = `${m}::Server`;
  const emptyCallbackType = (id: string): ProviderTypeExpr => ({
    kind: "function",
    id,
    parameters: [],
    returnType: voidType,
  });
  const requestCallbackType: ProviderTypeExpr = {
    kind: "function",
    id: `${m}.RequestCallback`,
    parameters: [
      { name: "request", type: providerRef(m, "IncomingMessage") },
      { name: "response", type: providerRef(m, "ServerResponse") },
    ],
    returnType: voidType,
  };
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.http",
    imports: [{
      moduleSpecifier: "node:buffer",
      namedImports: [{ exportedName: "Buffer" }],
    }],
    exports: [
      {
        id: incomingId,
        name: "IncomingMessage",
        kind: "class",
        members: [propertyMember(incomingId, "url", stringType)],
      },
      {
        id: responseId,
        name: "ServerResponse",
        kind: "class",
        members: [
          propertyMember(responseId, "statusCode", int32Type, { readonly: false }),
          methodMember(responseId, "setHeader", [
            { name: "name", type: stringType },
            { name: "value", type: stringType },
          ], voidType),
          {
            id: `${responseId}.end`,
            name: "end",
            kind: "method",
            signatures: [
              { id: `${responseId}.end()`, parameters: [], returnType: voidType },
              {
                id: `${responseId}.end(string)`,
                parameters: [{ name: "chunk", type: stringType }],
                returnType: voidType,
              },
              {
                id: `${responseId}.end(buffer)`,
                parameters: [{ name: "chunk", type: providerRef("node:buffer", "Buffer") }],
                returnType: voidType,
              },
            ],
          },
        ],
      },
      {
        id: serverId,
        name: "Server",
        kind: "class",
        members: [{
          id: `${serverId}.listen`,
          name: "listen",
          kind: "method",
          signatures: [
            {
              id: `${serverId}.listen(port,callback)`,
              parameters: [
                { name: "port", type: int32Type },
                { name: "callback", type: emptyCallbackType(`${serverId}.listen(port,callback).callback`) },
              ],
              returnType: providerRef(m, "Server"),
            },
            {
              id: `${serverId}.listen(port,host,callback)`,
              parameters: [
                { name: "port", type: int32Type },
                { name: "host", type: stringType },
                { name: "callback", type: emptyCallbackType(`${serverId}.listen(port,host,callback).callback`) },
              ],
              returnType: providerRef(m, "Server"),
            },
          ],
        }],
      },
      fnExport(m, "createServer", [{ name: "handler", type: requestCallbackType }], providerRef(m, "Server")),
    ],
  };
}

export function httpRows(): readonly RustProviderOperationDefinition[] {
  const m = "node:http";
  const incomingId = `${m}::IncomingMessage`;
  const responseId = `${m}::ServerResponse`;
  const serverId = `${m}::Server`;
  return [
    {
      exportId: `${m}::createServer`,
      operationKind: "method",
      target: { form: "call", path: "node_http::create_server_callable" },
      resultCarrier: httpServerCarrier,
      parameterCarriers: [httpRequestCallbackCarrier],
    },
    {
      exportId: incomingId,
      memberId: `${incomingId}.url`,
      operationKind: "property",
      target: { form: "receiver-method", name: "url" },
      resultCarrier: stringCarrier,
    },
    {
      exportId: responseId,
      memberId: `${responseId}.statusCode`,
      operationKind: "property",
      target: { form: "receiver-method", name: "status_code" },
      resultCarrier: int32Carrier,
    },
    {
      exportId: responseId,
      memberId: `${responseId}.statusCode`,
      operationKind: "property-set",
      target: { form: "receiver-method", name: "set_status_code" },
      resultCarrier: unitCarrier,
      parameterCarriers: [int32Carrier],
    },
    {
      exportId: responseId,
      memberId: `${responseId}.setHeader`,
      operationKind: "method",
      target: { form: "receiver-method", name: "set_header", argModes: ["ref", "ref"] },
      resultCarrier: unitCarrier,
      parameterCarriers: [stringCarrier, stringCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: responseId,
      memberId: `${responseId}.end`,
      signatureId: `${responseId}.end()`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end_empty" },
      resultCarrier: unitCarrier,
    },
    {
      exportId: responseId,
      memberId: `${responseId}.end`,
      signatureId: `${responseId}.end(string)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end_string", argModes: ["ref"] },
      resultCarrier: unitCarrier,
      parameterCarriers: [stringCarrier],
    },
    {
      exportId: responseId,
      memberId: `${responseId}.end`,
      signatureId: `${responseId}.end(buffer)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end_buffer" },
      resultCarrier: unitCarrier,
      parameterCarriers: [bufferCarrier],
    },
    {
      exportId: serverId,
      memberId: `${serverId}.listen`,
      signatureId: `${serverId}.listen(port,callback)`,
      operationKind: "method",
      target: {
        form: "receiver-method",
        name: "listen_default_host",
        argModes: ["value", "value"],
      },
      resultCarrier: httpServerCarrier,
      parameterCarriers: [int32Carrier, emptyCallbackCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: serverId,
      memberId: `${serverId}.listen`,
      signatureId: `${serverId}.listen(port,host,callback)`,
      operationKind: "method",
      target: {
        form: "receiver-method",
        name: "listen",
        argModes: ["value", "ref", "value"],
      },
      resultCarrier: httpServerCarrier,
      parameterCarriers: [int32Carrier, stringCarrier, emptyCallbackCarrier],
      ...providerNativeFallibility,
    },
  ];
}

// --- node:timers -------------------------------------------------------------
