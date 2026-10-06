import { boolCarrier, emptyCallbackCarrier, stringArrayCarrier, stringCarrier, unitCarrier } from "../../model/carriers.js";
import { booleanType, stringArrayType, stringType, voidType } from "../../model/source-types.js";
import { httpsClientRequestCarrier, httpsServerCarrier } from "./carriers.js";
import { httpResponseCallbackCarrier, httpRequestCallbackCarrier } from "../http/carriers.js";
import { propertyMember, providerCallbackType, providerRef } from "../../declarations/builders.js";
import { providerNativeFallibility } from "../../model/operations.js";
import { nodeBackgroundInput, nodeTlsInput, nodeHttpInput, nodeRuntimeTaskInput } from "../../model/dispatch.js";
import { rustOptionTargetType } from "@tsonic/target-rust/provider";
import { tlsServerOptionsCarrier } from "../tls/carriers.js";
import type { ProviderTypeExpr } from "../../model/source-types.js";
import type { RustProviderModuleDefinition, RustProviderOperationDefinition, RustTargetTypeRef } from "@tsonic/target-rust/provider";

const moduleSpecifier = "node:https";
const optionsId = `${moduleSpecifier}::ServerOptions`;
const serverId = `${moduleSpecifier}::Server`;
const clientRequestId = `${moduleSpecifier}::ClientRequest`;
const optionsType = providerRef(moduleSpecifier, "ServerOptions");
const serverType = providerRef(moduleSpecifier, "Server");
const clientRequestType = providerRef(moduleSpecifier, "ClientRequest");
const requestCallbackType = (signatureId: string): ProviderTypeExpr =>
  providerCallbackType(signatureId, "handler", [
    { name: "request", type: providerRef("node:http", "IncomingMessage") },
    { name: "response", type: providerRef("node:http", "ServerResponse") },
  ]);
const responseCallbackType = (signatureId: string): ProviderTypeExpr =>
  providerCallbackType(signatureId, "callback", [
    { name: "response", type: providerRef("node:http", "IncomingMessage") },
  ]);

export function httpsModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.https",
    imports: [{
      moduleSpecifier: "node:http",
      namedImports: [
        { exportedName: "IncomingMessage" },
        { exportedName: "ServerResponse" },
      ],
    }],
    exports: [
      {
        id: optionsId,
        name: "ServerOptions",
        kind: "interface",
        members: [
          propertyMember(optionsId, "key", stringType, { readonly: false, optional: true }),
          propertyMember(optionsId, "cert", stringType, { readonly: false, optional: true }),
          propertyMember(optionsId, "ca", stringArrayType, { readonly: false, optional: true }),
          propertyMember(optionsId, "ALPNProtocols", stringArrayType, { readonly: false, optional: true }),
          propertyMember(optionsId, "requestCert", booleanType, { readonly: false, optional: true }),
          propertyMember(optionsId, "rejectUnauthorized", booleanType, { readonly: false, optional: true }),
        ],
      },
      {
        id: serverId,
        name: "Server",
        kind: "class",
        members: [
          {
            id: `${serverId}.listen`,
            name: "listen",
            kind: "method",
            signatures: [
              {
                id: `${serverId}.listen(port,callback)`,
                parameters: [
                  { name: "port", type: { kind: "number" } },
                  { name: "callback", type: providerCallbackType(`${serverId}.listen(port,callback)`, "callback", []) },
                ],
                returnType: serverType,
              },
              {
                id: `${serverId}.listen(port,host,callback)`,
                parameters: [
                  { name: "port", type: { kind: "number" } },
                  { name: "host", type: stringType },
                  { name: "callback", type: providerCallbackType(`${serverId}.listen(port,host,callback)`, "callback", []) },
                ],
                returnType: serverType,
              },
            ],
          },
          method(serverId, "close", voidType),
          method(serverId, "ref", serverType),
          method(serverId, "unref", serverType),
          propertyMember(serverId, "listening", booleanType),
        ],
      },
      {
        id: clientRequestId,
        name: "ClientRequest",
        kind: "class",
        members: [
          method(clientRequestId, "write", booleanType, [{ name: "chunk", type: stringType }]),
          method(clientRequestId, "end", voidType),
        ],
      },
      {
        id: `${moduleSpecifier}::createServer`,
        name: "createServer",
        kind: "function",
        signatures: [{
          id: `${moduleSpecifier}::createServer(options,handler)`,
          parameters: [
            { name: "options", type: optionsType },
            { name: "handler", type: requestCallbackType(`${moduleSpecifier}::createServer(options,handler)`) },
          ],
          returnType: serverType,
        }],
      },
      {
        id: `${moduleSpecifier}::request`,
        name: "request",
        kind: "function",
        signatures: [{
          id: `${moduleSpecifier}::request(url,callback)`,
          parameters: [
            { name: "url", type: stringType },
            { name: "callback", type: responseCallbackType(`${moduleSpecifier}::request(url,callback)`) },
          ],
          returnType: clientRequestType,
        }],
      },
      {
        id: `${moduleSpecifier}::get`,
        name: "get",
        kind: "function",
        signatures: [{
          id: `${moduleSpecifier}::get(url,callback)`,
          parameters: [
            { name: "url", type: stringType },
            { name: "callback", type: responseCallbackType(`${moduleSpecifier}::get(url,callback)`) },
          ],
          returnType: clientRequestType,
        }],
      },
    ],
  };
}

export function httpsRows(): readonly RustProviderOperationDefinition[] {
  const serverResult = httpsServerCarrier;
  return [
    ...optionRows(),
    {
      exportId: `${moduleSpecifier}::createServer`,
      operationKind: "method",
      target: { form: "call", path: "node_https::create_server_callable", argModes: ["value", "value"] },
      resultCarrier: httpsServerCarrier,
      parameterCarriers: [tlsServerOptionsCarrier, httpRequestCallbackCarrier],
      dispatchInputs: [nodeHttpInput, { ...nodeTlsInput, targetArgumentIndex: 1 }, { ...nodeBackgroundInput, targetArgumentIndex: 2 }],
      ...providerNativeFallibility,
    },
    {
      exportId: `${moduleSpecifier}::request`,
      operationKind: "method",
      target: { form: "call", path: "node_https::request_callable", argModes: ["ref", "value"] },
      resultCarrier: httpsClientRequestCarrier,
      parameterCarriers: [stringCarrier, httpResponseCallbackCarrier],
      dispatchInputs: [nodeBackgroundInput],
      ...providerNativeFallibility,
    },
    {
      exportId: `${moduleSpecifier}::get`,
      operationKind: "method",
      target: { form: "call", path: "node_https::get_callable", argModes: ["ref", "value"] },
      resultCarrier: httpsClientRequestCarrier,
      parameterCarriers: [stringCarrier, httpResponseCallbackCarrier],
      dispatchInputs: [nodeBackgroundInput],
      ...providerNativeFallibility,
    },
    {
      exportId: clientRequestId,
      memberId: `${clientRequestId}.write`,
      operationKind: "method",
      target: { form: "receiver-method", name: "write_string", argModes: ["ref"] },
      resultCarrier: boolCarrier,
      receiverCarrier: httpsClientRequestCarrier,
      parameterCarriers: [stringCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: clientRequestId,
      memberId: `${clientRequestId}.end`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end" },
      resultCarrier: unitCarrier,
      receiverCarrier: httpsClientRequestCarrier,
      parameterCarriers: [],
      ...providerNativeFallibility,
    },
    { ...serverMethod("listen", "port,callback", "listen_default_host", [{ kind: "type-parameter", identity: "node:https:numeric:Port", name: "Port" }, emptyCallbackCarrier], ["value", "value"], serverResult, true), genericParameters: [{ kind: "type", targetIdentity: "node:https:numeric:Port", sourceName: "Port" }], dispatchInputs: [nodeRuntimeTaskInput] },
    { ...serverMethod("listen", "port,host,callback", "listen", [{ kind: "type-parameter", identity: "node:https:numeric:Port", name: "Port" }, stringCarrier, emptyCallbackCarrier], ["value", "ref", "value"], serverResult, true), genericParameters: [{ kind: "type", targetIdentity: "node:https:numeric:Port", sourceName: "Port" }], dispatchInputs: [nodeRuntimeTaskInput] },
    serverMethod("close", undefined, "close", [], [], unitCarrier, false),
    serverMethod("ref", undefined, "ref_chain", [], [], serverResult, false),
    serverMethod("unref", undefined, "unref_chain", [], [], serverResult, false),
    {
      exportId: serverId,
      memberId: `${serverId}.listening`,
      operationKind: "property",
      target: { form: "receiver-method", name: "listening" },
      resultCarrier: boolCarrier,
      receiverCarrier: httpsServerCarrier,
    },
  ];
}

function method(
  ownerId: string,
  name: string,
  returnType: ProviderTypeExpr,
  parameters: readonly { readonly name: string; readonly type: ProviderTypeExpr }[] = [],
) {
  return {
    id: `${ownerId}.${name}`,
    name,
    kind: "method" as const,
    signatures: [{
      id: `${ownerId}.${name}()`,
      parameters,
      returnType,
    }],
  };
}

function optionRows(): readonly RustProviderOperationDefinition[] {
  const fields = [
    ["key", "key", rustOptionTargetType(stringCarrier)],
    ["cert", "cert", rustOptionTargetType(stringCarrier)],
    ["ca", "ca", rustOptionTargetType(stringArrayCarrier)],
    ["ALPNProtocols", "alpn_protocols", rustOptionTargetType(stringArrayCarrier)],
    ["requestCert", "request_cert", rustOptionTargetType(boolCarrier)],
    ["rejectUnauthorized", "reject_unauthorized", rustOptionTargetType(boolCarrier)],
  ] as const;
  return fields.flatMap(([sourceName, targetName, carrier]) => [
    {
      exportId: optionsId,
      memberId: `${optionsId}.${sourceName}`,
      operationKind: "property" as const,
      target: { form: "field" as const, name: targetName },
      resultCarrier: carrier,
      receiverCarrier: tlsServerOptionsCarrier,
    },
    {
      exportId: optionsId,
      memberId: `${optionsId}.${sourceName}`,
      operationKind: "property-set" as const,
      target: { form: "field" as const, name: targetName },
      resultCarrier: unitCarrier,
      receiverCarrier: tlsServerOptionsCarrier,
      parameterCarriers: [carrier],
    },
  ]);
}

function serverMethod(
  member: string,
  signature: string | undefined,
  name: string,
  parameters: readonly RustTargetTypeRef[],
  argModes: readonly ("value" | "ref" | "mut-ref")[],
  resultCarrier: RustTargetTypeRef,
  fallible: boolean,
): RustProviderOperationDefinition {
  const operation = {
    exportId: serverId,
    memberId: `${serverId}.${member}`,
    ...(signature === undefined ? {} : { signatureId: `${serverId}.${member}(${signature})` }),
    operationKind: "method",
    target: {
      form: "receiver-method",
      name,
      ...(argModes.length === 0 ? {} : { argModes }),
    },
    resultCarrier,
    receiverCarrier: httpsServerCarrier,
    parameterCarriers: parameters,
  } as const;
  return fallible
    ? { ...operation, ...providerNativeFallibility }
    : operation;
}
