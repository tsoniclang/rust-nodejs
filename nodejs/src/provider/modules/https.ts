import {
  boolCarrier,
  booleanType,
  emptyCallbackCarrier,
  float64Carrier,
  httpsClientRequestCarrier,
  httpsServerCarrier,
  httpResponseCallbackCarrier,
  httpRequestCallbackCarrier,
  propertyMember,
  providerNativeFallibility,
  providerRef,
  rustOptionTargetType,
  stringArrayCarrier,
  stringArrayType,
  stringCarrier,
  stringType,
  tlsServerOptionsCarrier,
  unitCarrier,
  voidType,
} from "../model.js";
import type {
  ProviderTypeExpr,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustTargetTypeRef,
} from "../model.js";

const moduleSpecifier = "node:https";
const optionsId = `${moduleSpecifier}::ServerOptions`;
const serverId = `${moduleSpecifier}::Server`;
const clientRequestId = `${moduleSpecifier}::ClientRequest`;
const optionsType = providerRef(moduleSpecifier, "ServerOptions");
const serverType = providerRef(moduleSpecifier, "Server");
const clientRequestType = providerRef(moduleSpecifier, "ClientRequest");
const emptyCallbackType: ProviderTypeExpr = {
  kind: "function",
  id: `${moduleSpecifier}.ListenCallback`,
  parameters: [],
  returnType: voidType,
};
const requestCallbackType: ProviderTypeExpr = {
  kind: "function",
  id: `${moduleSpecifier}.RequestListener`,
  parameters: [
    { name: "request", type: providerRef("node:http", "IncomingMessage") },
    { name: "response", type: providerRef("node:http", "ServerResponse") },
  ],
  returnType: voidType,
};
const responseCallbackType: ProviderTypeExpr = {
  kind: "function",
  id: `${moduleSpecifier}.ResponseListener`,
  parameters: [{ name: "response", type: providerRef("node:http", "IncomingMessage") }],
  returnType: voidType,
};

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
                  { name: "callback", type: emptyCallbackType },
                ],
                returnType: serverType,
              },
              {
                id: `${serverId}.listen(port,host,callback)`,
                parameters: [
                  { name: "port", type: { kind: "number" } },
                  { name: "host", type: stringType },
                  { name: "callback", type: emptyCallbackType },
                ],
                returnType: serverType,
              },
            ],
          },
          method("close", voidType),
          method("ref", serverType),
          method("unref", serverType),
          propertyMember(serverId, "listening", booleanType),
        ],
      },
      {
        id: clientRequestId,
        name: "ClientRequest",
        kind: "class",
        members: [
          method("write", booleanType, [{ name: "chunk", type: stringType }]),
          method("end", voidType),
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
            { name: "handler", type: requestCallbackType },
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
            { name: "callback", type: responseCallbackType },
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
            { name: "callback", type: responseCallbackType },
          ],
          returnType: clientRequestType,
        }],
      },
    ],
  };
}

export function httpsRows(): readonly RustProviderOperationDefinition[] {
  const mutableServer: RustTargetTypeRef = {
    kind: "reference",
    referent: httpsServerCarrier,
    mutable: true,
  };
  return [
    ...optionRows(),
    {
      exportId: `${moduleSpecifier}::createServer`,
      operationKind: "method",
      target: { form: "call", path: "node_https::create_server_callable", argModes: ["value", "value"] },
      resultCarrier: httpsServerCarrier,
      parameterCarriers: [tlsServerOptionsCarrier, httpRequestCallbackCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: `${moduleSpecifier}::request`,
      operationKind: "method",
      target: { form: "call", path: "node_https::request_callable", argModes: ["ref", "value"] },
      resultCarrier: httpsClientRequestCarrier,
      parameterCarriers: [stringCarrier, httpResponseCallbackCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: `${moduleSpecifier}::get`,
      operationKind: "method",
      target: { form: "call", path: "node_https::get_callable", argModes: ["ref", "value"] },
      resultCarrier: httpsClientRequestCarrier,
      parameterCarriers: [stringCarrier, httpResponseCallbackCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: clientRequestId,
      memberId: `${clientRequestId}.write`,
      operationKind: "method",
      target: { form: "receiver-method", name: "write_string", argModes: ["ref"], mutatesReceiver: true },
      resultCarrier: boolCarrier,
      receiverCarrier: httpsClientRequestCarrier,
      parameterCarriers: [stringCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: clientRequestId,
      memberId: `${clientRequestId}.end`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end", mutatesReceiver: true },
      resultCarrier: unitCarrier,
      receiverCarrier: httpsClientRequestCarrier,
      parameterCarriers: [],
      ...providerNativeFallibility,
    },
    serverMethod("listen", "port,callback", "listen_default_host", [float64Carrier, emptyCallbackCarrier], mutableServer, true),
    serverMethod("listen", "port,host,callback", "listen", [float64Carrier, stringCarrier, emptyCallbackCarrier], mutableServer, true),
    serverMethod("close", undefined, "close", [], unitCarrier, false),
    serverMethod("ref", undefined, "ref_chain", [], mutableServer, false),
    serverMethod("unref", undefined, "unref_chain", [], mutableServer, false),
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
  name: string,
  returnType: ProviderTypeExpr,
  parameters: readonly { readonly name: string; readonly type: ProviderTypeExpr }[] = [],
) {
  return {
    id: `${serverId}.${name}`,
    name,
    kind: "method" as const,
    signatures: [{
      id: `${serverId}.${name}()`,
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
  resultCarrier: RustTargetTypeRef,
  fallible: boolean,
): RustProviderOperationDefinition {
  return {
    exportId: serverId,
    memberId: `${serverId}.${member}`,
    ...(signature === undefined ? {} : { signatureId: `${serverId}.${member}(${signature})` }),
    operationKind: "method",
    target: { form: "receiver-method", name, mutatesReceiver: true },
    resultCarrier,
    receiverCarrier: httpsServerCarrier,
    parameterCarriers: parameters,
    ...(fallible ? providerNativeFallibility : {}),
  };
}
