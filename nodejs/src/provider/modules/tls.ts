import {
  boolCarrier,
  booleanType,
  emptyCallbackCarrier,
  float64Carrier,
  numberType,
  propertyMember,
  providerNativeFallibility,
  providerRef,
  rustOptionTargetType,
  stringArrayCarrier,
  stringArrayType,
  stringCarrier,
  stringType,
  tlsConnectOptionsCarrier,
  tlsServerCarrier,
  tlsServerOptionsCarrier,
  tlsSocketCallbackCarrier,
  tlsSocketCarrier,
  unitCarrier,
  undefinedType,
  voidType,
} from "../model.js";
import type {
  ProviderTypeExpr,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustTargetTypeRef,
} from "../model.js";

const moduleSpecifier = "node:tls";
const connectOptionsId = `${moduleSpecifier}::ConnectionOptions`;
const serverOptionsId = `${moduleSpecifier}::TlsOptions`;
const socketId = `${moduleSpecifier}::TLSSocket`;
const serverId = `${moduleSpecifier}::Server`;
const connectOptionsType = providerRef(moduleSpecifier, "ConnectionOptions");
const serverOptionsType = providerRef(moduleSpecifier, "TlsOptions");
const socketType = providerRef(moduleSpecifier, "TLSSocket");
const serverType = providerRef(moduleSpecifier, "Server");
const emptyCallbackType: ProviderTypeExpr = {
  kind: "function",
  id: `${moduleSpecifier}.SecureConnectCallback`,
  parameters: [],
  returnType: voidType,
};
const socketCallbackType: ProviderTypeExpr = {
  kind: "function",
  id: `${moduleSpecifier}.SecureConnectionCallback`,
  parameters: [{ name: "socket", type: socketType }],
  returnType: voidType,
};

export function tlsModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.tls",
    exports: [
      optionsDeclaration(connectOptionsId, "ConnectionOptions", [
        ["host", stringType],
        ["servername", stringType],
        ["port", numberType],
        ["ALPNProtocols", stringArrayType],
        ["rejectUnauthorized", booleanType],
        ["ca", stringArrayType],
        ["timeout", numberType],
      ]),
      optionsDeclaration(serverOptionsId, "TlsOptions", [
        ["key", stringType],
        ["cert", stringType],
        ["ca", stringArrayType],
        ["ALPNProtocols", stringArrayType],
        ["requestCert", booleanType],
        ["rejectUnauthorized", booleanType],
      ]),
      {
        id: socketId,
        name: "TLSSocket",
        kind: "class",
        members: [
          method(socketId, "write", "buffer", providerRef("node:buffer", "Buffer"), booleanType),
          method(socketId, "write", "string", stringType, booleanType),
          method(socketId, "read", "", undefined, {
            kind: "union",
            types: [providerRef("node:buffer", "Buffer"), undefinedType],
          }),
          method(socketId, "end", "", undefined, voidType),
          method(socketId, "ref", "", undefined, socketType),
          method(socketId, "unref", "", undefined, socketType),
          propertyMember(socketId, "authorized", booleanType),
          propertyMember(socketId, "authorizationError", {
            kind: "union",
            types: [stringType, { kind: "undefined" }],
          }),
          propertyMember(socketId, "encrypted", booleanType),
          propertyMember(socketId, "servername", stringType),
          propertyMember(socketId, "alpnProtocol", {
            kind: "union",
            types: [stringType, { kind: "undefined" }],
          }),
          propertyMember(socketId, "bytesRead", numberType),
          propertyMember(socketId, "bytesWritten", numberType),
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
                  { name: "port", type: numberType },
                  { name: "callback", type: emptyCallbackType },
                ],
                returnType: serverType,
              },
              {
                id: `${serverId}.listen(port,host,callback)`,
                parameters: [
                  { name: "port", type: numberType },
                  { name: "host", type: stringType },
                  { name: "callback", type: emptyCallbackType },
                ],
                returnType: serverType,
              },
            ],
          },
          method(serverId, "close", "", undefined, voidType),
          method(serverId, "ref", "", undefined, serverType),
          method(serverId, "unref", "", undefined, serverType),
          propertyMember(serverId, "listening", booleanType),
        ],
      },
      {
        id: `${moduleSpecifier}::connect`,
        name: "connect",
        kind: "function",
        signatures: [
          {
            id: `${moduleSpecifier}::connect(options)`,
            parameters: [{ name: "options", type: connectOptionsType }],
            returnType: socketType,
          },
          {
            id: `${moduleSpecifier}::connect(options,callback)`,
            parameters: [
              { name: "options", type: connectOptionsType },
              { name: "callback", type: emptyCallbackType },
            ],
            returnType: socketType,
          },
        ],
      },
      {
        id: `${moduleSpecifier}::createServer`,
        name: "createServer",
        kind: "function",
        signatures: [{
          id: `${moduleSpecifier}::createServer(options,callback)`,
          parameters: [
            { name: "options", type: serverOptionsType },
            { name: "callback", type: socketCallbackType },
          ],
          returnType: serverType,
        }],
      },
    ],
    imports: [{
      moduleSpecifier: "node:buffer",
      namedImports: [{ exportedName: "Buffer" }],
    }],
  };
}

export function tlsRows(): readonly RustProviderOperationDefinition[] {
  const mutableSocket: RustTargetTypeRef = { kind: "reference", referent: tlsSocketCarrier, mutable: true };
  const mutableServer: RustTargetTypeRef = { kind: "reference", referent: tlsServerCarrier, mutable: true };
  return [
    ...optionRows(connectOptionsId, tlsConnectOptionsCarrier, [
      ["host", "host", rustOptionTargetType(stringCarrier)],
      ["servername", "servername", rustOptionTargetType(stringCarrier)],
      ["port", "port", rustOptionTargetType(float64Carrier)],
      ["ALPNProtocols", "alpn_protocols", rustOptionTargetType(stringArrayCarrier)],
      ["rejectUnauthorized", "reject_unauthorized", rustOptionTargetType(boolCarrier)],
      ["ca", "ca", rustOptionTargetType(stringArrayCarrier)],
      ["timeout", "timeout", rustOptionTargetType(float64Carrier)],
    ]),
    ...optionRows(serverOptionsId, tlsServerOptionsCarrier, [
      ["key", "key", rustOptionTargetType(stringCarrier)],
      ["cert", "cert", rustOptionTargetType(stringCarrier)],
      ["ca", "ca", rustOptionTargetType(stringArrayCarrier)],
      ["ALPNProtocols", "alpn_protocols", rustOptionTargetType(stringArrayCarrier)],
      ["requestCert", "request_cert", rustOptionTargetType(boolCarrier)],
      ["rejectUnauthorized", "reject_unauthorized", rustOptionTargetType(boolCarrier)],
    ]),
    {
      exportId: `${moduleSpecifier}::connect`,
      signatureId: `${moduleSpecifier}::connect(options)`,
      operationKind: "method",
      target: { form: "call", path: "node_tls::connect", argModes: ["value"] },
      resultCarrier: tlsSocketCarrier,
      parameterCarriers: [tlsConnectOptionsCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: `${moduleSpecifier}::connect`,
      signatureId: `${moduleSpecifier}::connect(options,callback)`,
      operationKind: "method",
      target: { form: "call", path: "node_tls::connect_callable", argModes: ["value", "value"] },
      resultCarrier: tlsSocketCarrier,
      parameterCarriers: [tlsConnectOptionsCarrier, emptyCallbackCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: `${moduleSpecifier}::createServer`,
      operationKind: "method",
      target: { form: "call", path: "node_tls::create_server", argModes: ["value", "value"] },
      resultCarrier: tlsServerCarrier,
      parameterCarriers: [tlsServerOptionsCarrier, tlsSocketCallbackCarrier],
      ...providerNativeFallibility,
    },
    socketMethod("write", "buffer", "write_buffer", [
      { kind: "target-named", id: "rust.node.Buffer" },
    ], boolCarrier, true),
    socketMethod("write", "string", "write_string", [stringCarrier], boolCarrier, true),
    socketMethod("read", undefined, "read_optional_buffer", [], rustOptionTargetType({ kind: "target-named", id: "rust.node.Buffer" }), true),
    socketMethod("end", undefined, "end", [], unitCarrier, true),
    socketMethod("ref", undefined, "ref_chain", [], mutableSocket, false),
    socketMethod("unref", undefined, "unref_chain", [], mutableSocket, false),
    socketProperty("authorized", "authorized", boolCarrier),
    socketProperty("authorizationError", "authorization_error", rustOptionTargetType(stringCarrier)),
    socketProperty("encrypted", "encrypted", boolCarrier),
    socketProperty("servername", "servername_string", stringCarrier),
    socketProperty("alpnProtocol", "alpn_protocol", rustOptionTargetType(stringCarrier)),
    socketProperty("bytesRead", "bytes_read_number", float64Carrier),
    socketProperty("bytesWritten", "bytes_written_number", float64Carrier),
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
      receiverCarrier: tlsServerCarrier,
    },
  ];
}

function optionsDeclaration(
  id: string,
  name: string,
  fields: readonly (readonly [string, ProviderTypeExpr])[],
) {
  return {
    id,
    name,
    kind: "interface" as const,
    members: fields.map(([field, type]) => propertyMember(id, field, type, {
      readonly: false,
      optional: true,
    })),
  };
}

function method(
  classId: string,
  name: string,
  signature: string,
  parameterType: ProviderTypeExpr | undefined,
  returnType: ProviderTypeExpr,
) {
  return {
    id: `${classId}.${name}`,
    name,
    kind: "method" as const,
    signatures: [{
      id: `${classId}.${name}(${signature})`,
      parameters: parameterType === undefined ? [] : [{ name: "value", type: parameterType }],
      returnType,
    }],
  };
}

function optionRows(
  exportId: string,
  receiverCarrier: RustTargetTypeRef,
  fields: readonly (readonly [string, string, RustTargetTypeRef])[],
): readonly RustProviderOperationDefinition[] {
  return fields.flatMap(([sourceName, targetName, carrier]) => [
    {
      exportId,
      memberId: `${exportId}.${sourceName}`,
      operationKind: "property" as const,
      target: { form: "field" as const, name: targetName },
      resultCarrier: carrier,
      receiverCarrier,
    },
    {
      exportId,
      memberId: `${exportId}.${sourceName}`,
      operationKind: "property-set" as const,
      target: { form: "field" as const, name: targetName },
      resultCarrier: unitCarrier,
      receiverCarrier,
      parameterCarriers: [carrier],
    },
  ]);
}

function socketMethod(
  member: string,
  signature: string | undefined,
  name: string,
  parameters: readonly RustTargetTypeRef[],
  resultCarrier: RustTargetTypeRef,
  fallible: boolean,
): RustProviderOperationDefinition {
  return {
    exportId: socketId,
    memberId: `${socketId}.${member}`,
    ...(signature === undefined ? {} : { signatureId: `${socketId}.${member}(${signature})` }),
    operationKind: "method",
    target: {
      form: "receiver-method",
      name,
      ...(parameters.length === 0 ? {} : { argModes: parameters.map(() => "value" as const) }),
      mutatesReceiver: true,
    },
    resultCarrier,
    receiverCarrier: tlsSocketCarrier,
    parameterCarriers: parameters,
    ...(fallible ? providerNativeFallibility : {}),
  };
}

function socketProperty(
  member: string,
  name: string,
  resultCarrier: RustTargetTypeRef,
): RustProviderOperationDefinition {
  return {
    exportId: socketId,
    memberId: `${socketId}.${member}`,
    operationKind: "property",
    target: { form: "receiver-method", name },
    resultCarrier,
    receiverCarrier: tlsSocketCarrier,
  };
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
    receiverCarrier: tlsServerCarrier,
    parameterCarriers: parameters,
    ...(fallible ? providerNativeFallibility : {}),
  };
}
