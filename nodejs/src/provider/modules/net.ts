import {
  boolCarrier,
  booleanType,
  bufferCarrier,
  emptyCallbackCarrier,
  netConnectionCallbackCarrier,
  netServerCarrier,
  netSocketCarrier,
  numberType,
  propertyMember,
  providerCallbackType,
  providerNativeFallibility,
  providerRef,
  stringCarrier,
  stringType,
  unitCarrier,
  voidType,
} from "../model.js";
import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";

const moduleSpecifier = "node:net";
const socketId = `${moduleSpecifier}::Socket`;
const serverId = `${moduleSpecifier}::Server`;
const bufferType = providerRef("node:buffer", "Buffer");

export function netModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.net",
    imports: [{
      moduleSpecifier: "node:buffer",
      namedImports: [{ exportedName: "Buffer" }],
    }],
    exports: [
      {
        id: socketId,
        name: "Socket",
        kind: "class",
        members: [
          {
            id: `${socketId}.write`,
            name: "write",
            kind: "method",
            signatures: [
              { id: `${socketId}.write(buffer)`, parameters: [{ name: "data", type: bufferType }], returnType: booleanType },
              { id: `${socketId}.write(string)`, parameters: [{ name: "data", type: stringType }], returnType: booleanType },
            ],
          },
          {
            id: `${socketId}.read`,
            name: "read",
            kind: "method",
            signatures: [{ id: `${socketId}.read()`, parameters: [], returnType: bufferType }],
          },
          {
            id: `${socketId}.end`,
            name: "end",
            kind: "method",
            signatures: [
              { id: `${socketId}.end()`, parameters: [], returnType: voidType },
              { id: `${socketId}.end(buffer)`, parameters: [{ name: "data", type: bufferType }], returnType: voidType },
              { id: `${socketId}.end(string)`, parameters: [{ name: "data", type: stringType }], returnType: voidType },
            ],
          },
          ...(["destroy", "ref", "unref", "pause", "resume"] as const).map((name) => ({
            id: `${socketId}.${name}`,
            name,
            kind: "method" as const,
            signatures: [{ id: `${socketId}.${name}()`, parameters: [], returnType: name === "destroy" ? voidType : providerRef(moduleSpecifier, "Socket") }],
          })),
          {
            id: `${socketId}.setNoDelay`,
            name: "setNoDelay",
            kind: "method",
            signatures: [{
              id: `${socketId}.setNoDelay(value)`,
              parameters: [{ name: "value", type: booleanType }],
              returnType: providerRef(moduleSpecifier, "Socket"),
            }],
          },
          {
            id: `${socketId}.setTimeout`,
            name: "setTimeout",
            kind: "method",
            signatures: [{
              id: `${socketId}.setTimeout(timeout)`,
              parameters: [{ name: "timeout", type: numberType }],
              returnType: providerRef(moduleSpecifier, "Socket"),
            }],
          },
          propertyMember(socketId, "bytesRead", numberType),
          propertyMember(socketId, "bytesWritten", numberType),
          propertyMember(socketId, "destroyed", booleanType),
          propertyMember(socketId, "pending", booleanType),
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
              { id: `${serverId}.listen(port)`, parameters: [{ name: "port", type: numberType }], returnType: providerRef(moduleSpecifier, "Server") },
              { id: `${serverId}.listen(port,host)`, parameters: [{ name: "port", type: numberType }, { name: "host", type: stringType }], returnType: providerRef(moduleSpecifier, "Server") },
              { id: `${serverId}.listen(port,callback)`, parameters: [{ name: "port", type: numberType }, { name: "callback", type: providerCallbackType(`${serverId}.listen(port,callback)`, "callback", []) }], returnType: providerRef(moduleSpecifier, "Server") },
              { id: `${serverId}.listen(port,host,callback)`, parameters: [{ name: "port", type: numberType }, { name: "host", type: stringType }, { name: "callback", type: providerCallbackType(`${serverId}.listen(port,host,callback)`, "callback", []) }], returnType: providerRef(moduleSpecifier, "Server") },
            ],
          },
          {
            id: `${serverId}.close`,
            name: "close",
            kind: "method",
            signatures: [{ id: `${serverId}.close()`, parameters: [], returnType: voidType }],
          },
          ...(["ref", "unref"] as const).map((name) => ({
            id: `${serverId}.${name}`,
            name,
            kind: "method" as const,
            signatures: [{ id: `${serverId}.${name}()`, parameters: [], returnType: providerRef(moduleSpecifier, "Server") }],
          })),
          propertyMember(serverId, "listening", booleanType),
        ],
      },
      {
        id: `${moduleSpecifier}::createConnection`,
        name: "createConnection",
        kind: "function",
        signatures: [
          { id: `${moduleSpecifier}::createConnection(port)`, name: "createConnection", parameters: [{ name: "port", type: numberType }], returnType: providerRef(moduleSpecifier, "Socket") },
          { id: `${moduleSpecifier}::createConnection(port,host)`, name: "createConnection", parameters: [{ name: "port", type: numberType }, { name: "host", type: stringType }], returnType: providerRef(moduleSpecifier, "Socket") },
          { id: `${moduleSpecifier}::createConnection(port,callback)`, name: "createConnection", parameters: [{ name: "port", type: numberType }, { name: "callback", type: providerCallbackType(`${moduleSpecifier}::createConnection(port,callback)`, "callback", []) }], returnType: providerRef(moduleSpecifier, "Socket") },
          { id: `${moduleSpecifier}::createConnection(port,host,callback)`, name: "createConnection", parameters: [{ name: "port", type: numberType }, { name: "host", type: stringType }, { name: "callback", type: providerCallbackType(`${moduleSpecifier}::createConnection(port,host,callback)`, "callback", []) }], returnType: providerRef(moduleSpecifier, "Socket") },
        ],
      },
      {
        id: `${moduleSpecifier}::createServer`,
        name: "createServer",
        kind: "function",
        signatures: [
          { id: `${moduleSpecifier}::createServer()`, name: "createServer", parameters: [], returnType: providerRef(moduleSpecifier, "Server") },
          { id: `${moduleSpecifier}::createServer(callback)`, name: "createServer", parameters: [{ name: "callback", type: providerCallbackType(`${moduleSpecifier}::createServer(callback)`, "callback", [{ name: "socket", type: providerRef(moduleSpecifier, "Socket") }]) }], returnType: providerRef(moduleSpecifier, "Server") },
        ],
      },
      ...(["isIP", "isIPv4", "isIPv6"] as const).map((name) => ({
        id: `${moduleSpecifier}::${name}`,
        name,
        kind: "function" as const,
        signatures: [{
          id: `${moduleSpecifier}::${name}(input)`,
          name,
          parameters: [{ name: "input", type: stringType }],
          returnType: name === "isIP" ? numberType : booleanType,
        }],
      })),
    ],
  };
}

export function netRows(): readonly RustProviderOperationDefinition[] {
  const mutableSocket = { kind: "reference", referent: netSocketCarrier, mutable: true } as const;
  const mutableServer = { kind: "reference", referent: netServerCarrier, mutable: true } as const;
  const rows: RustProviderOperationDefinition[] = [
    ...connectionRows(),
    {
      exportId: `${moduleSpecifier}::createServer`,
      signatureId: `${moduleSpecifier}::createServer()`,
      operationKind: "method",
      target: { form: "call", path: "node_net::create_server" },
      resultCarrier: netServerCarrier,
      parameterCarriers: [],
    },
    {
      exportId: `${moduleSpecifier}::createServer`,
      signatureId: `${moduleSpecifier}::createServer(callback)`,
      operationKind: "method",
      target: { form: "call", path: "node_net::create_server_callable", argModes: ["value"] },
      resultCarrier: netServerCarrier,
      parameterCarriers: [netConnectionCallbackCarrier],
    },
    ...(["isIPv4", "isIPv6"] as const).map((name): RustProviderOperationDefinition => ({
      exportId: `${moduleSpecifier}::${name}`,
      operationKind: "method",
      target: { form: "call", path: `node_net::${name === "isIPv4" ? "is_ipv4" : "is_ipv6"}`, argModes: ["ref"] },
      resultCarrier: boolCarrier,
      parameterCarriers: [stringCarrier],
    })),
    {
      exportId: `${moduleSpecifier}::isIP`,
      operationKind: "method",
      target: { form: "call", path: "node_net::is_ip_number", argModes: ["ref"] },
      resultCarrier: { kind: "source-primitive", name: "float64" },
      parameterCarriers: [stringCarrier],
    },
  ];
  const socketMethods = [
    ["write", "buffer", "write_buffer", bufferCarrier, boolCarrier, true],
    ["write", "string", "write_string", stringCarrier, boolCarrier, true],
    ["end", "buffer", "end_buffer", bufferCarrier, unitCarrier, true],
    ["end", "string", "end_string", stringCarrier, unitCarrier, true],
  ] as const;
  for (const [member, signature, target, parameter, result] of socketMethods) {
    rows.push({
      exportId: socketId,
      memberId: `${socketId}.${member}`,
      signatureId: `${socketId}.${member}(${signature})`,
      operationKind: "method",
      target: { form: "receiver-method", name: target, argModes: ["ref"], mutatesReceiver: true },
      resultCarrier: result,
      receiverCarrier: netSocketCarrier,
      parameterCarriers: [parameter],
      ...providerNativeFallibility,
    });
  }
  rows.push(
    { exportId: socketId, memberId: `${socketId}.read`, operationKind: "method", target: { form: "receiver-method", name: "read_buffer", mutatesReceiver: true }, resultCarrier: bufferCarrier, receiverCarrier: netSocketCarrier, parameterCarriers: [], ...providerNativeFallibility },
    { exportId: socketId, memberId: `${socketId}.end`, signatureId: `${socketId}.end()`, operationKind: "method", target: { form: "receiver-method", name: "end_empty", mutatesReceiver: true }, resultCarrier: unitCarrier, receiverCarrier: netSocketCarrier, parameterCarriers: [], ...providerNativeFallibility },
    { exportId: socketId, memberId: `${socketId}.destroy`, operationKind: "method", target: { form: "receiver-method", name: "destroy", mutatesReceiver: true }, resultCarrier: unitCarrier, receiverCarrier: netSocketCarrier, parameterCarriers: [], ...providerNativeFallibility },
    { exportId: socketId, memberId: `${socketId}.ref`, operationKind: "method", target: { form: "receiver-method", name: "ref_chain", mutatesReceiver: true }, resultCarrier: mutableSocket, receiverCarrier: netSocketCarrier, parameterCarriers: [] },
    { exportId: socketId, memberId: `${socketId}.unref`, operationKind: "method", target: { form: "receiver-method", name: "unref_chain", mutatesReceiver: true }, resultCarrier: mutableSocket, receiverCarrier: netSocketCarrier, parameterCarriers: [] },
    { exportId: socketId, memberId: `${socketId}.pause`, operationKind: "method", target: { form: "receiver-method", name: "pause_chain", mutatesReceiver: true }, resultCarrier: mutableSocket, receiverCarrier: netSocketCarrier, parameterCarriers: [] },
    { exportId: socketId, memberId: `${socketId}.resume`, operationKind: "method", target: { form: "receiver-method", name: "resume_chain", mutatesReceiver: true }, resultCarrier: mutableSocket, receiverCarrier: netSocketCarrier, parameterCarriers: [] },
    { exportId: socketId, memberId: `${socketId}.setNoDelay`, operationKind: "method", target: { form: "receiver-method", name: "set_no_delay_chain", argModes: ["value"], mutatesReceiver: true }, resultCarrier: mutableSocket, receiverCarrier: netSocketCarrier, parameterCarriers: [boolCarrier], ...providerNativeFallibility },
    { exportId: socketId, memberId: `${socketId}.setTimeout`, operationKind: "method", target: { form: "receiver-method", name: "set_timeout_number", argModes: ["value"], mutatesReceiver: true }, resultCarrier: mutableSocket, receiverCarrier: netSocketCarrier, parameterCarriers: [{ kind: "source-primitive", name: "float64" }], ...providerNativeFallibility },
    { exportId: serverId, memberId: `${serverId}.close`, operationKind: "method", target: { form: "receiver-method", name: "close", mutatesReceiver: true }, resultCarrier: unitCarrier, receiverCarrier: netServerCarrier, parameterCarriers: [] },
    { exportId: serverId, memberId: `${serverId}.ref`, operationKind: "method", target: { form: "receiver-method", name: "ref_chain", mutatesReceiver: true }, resultCarrier: mutableServer, receiverCarrier: netServerCarrier, parameterCarriers: [] },
    { exportId: serverId, memberId: `${serverId}.unref`, operationKind: "method", target: { form: "receiver-method", name: "unref_chain", mutatesReceiver: true }, resultCarrier: mutableServer, receiverCarrier: netServerCarrier, parameterCarriers: [] },
  );
  const listenRows = [
    ["port", "listen_port", [{ kind: "source-primitive", name: "float64" }], ["value"]],
    ["port,host", "listen_port_host", [{ kind: "source-primitive", name: "float64" }, stringCarrier], ["value", "ref"]],
    ["port,callback", "listen_port_callable", [{ kind: "source-primitive", name: "float64" }, emptyCallbackCarrier], ["value", "value"]],
    ["port,host,callback", "listen_port_host_callable", [{ kind: "source-primitive", name: "float64" }, stringCarrier, emptyCallbackCarrier], ["value", "ref", "value"]],
  ] as const;
  for (const [signature, target, parameterCarriers, argModes] of listenRows) {
    rows.push({
      exportId: serverId,
      memberId: `${serverId}.listen`,
      signatureId: `${serverId}.listen(${signature})`,
      operationKind: "method",
      target: { form: "receiver-method", name: target, argModes, mutatesReceiver: true },
      resultCarrier: mutableServer,
      receiverCarrier: netServerCarrier,
      parameterCarriers,
      ...providerNativeFallibility,
    });
  }
  for (const [memberName, methodName, resultCarrier] of [
    ["bytesRead", "bytes_read_number", { kind: "source-primitive", name: "float64" }],
    ["bytesWritten", "bytes_written_number", { kind: "source-primitive", name: "float64" }],
    ["destroyed", "destroyed", boolCarrier],
    ["pending", "pending", boolCarrier],
  ] as const) {
    rows.push({ exportId: socketId, memberId: `${socketId}.${memberName}`, operationKind: "property", target: { form: "receiver-method", name: methodName }, resultCarrier, receiverCarrier: netSocketCarrier });
  }
  rows.push({ exportId: serverId, memberId: `${serverId}.listening`, operationKind: "property", target: { form: "receiver-method", name: "listening" }, resultCarrier: boolCarrier, receiverCarrier: netServerCarrier });
  return rows;
}

function connectionRows(): readonly RustProviderOperationDefinition[] {
  const rows = [
    ["port", "create_connection_default_host", [{ kind: "source-primitive", name: "float64" }], ["value"]],
    ["port,host", "create_connection_source", [{ kind: "source-primitive", name: "float64" }, stringCarrier], ["value", "ref"]],
    ["port,callback", "create_connection_default_host_callable", [{ kind: "source-primitive", name: "float64" }, emptyCallbackCarrier], ["value", "value"]],
    ["port,host,callback", "create_connection_callable", [{ kind: "source-primitive", name: "float64" }, stringCarrier, emptyCallbackCarrier], ["value", "ref", "value"]],
  ] as const;
  return rows.map(([signature, path, parameterCarriers, argModes]) => ({
    exportId: `${moduleSpecifier}::createConnection`,
    signatureId: `${moduleSpecifier}::createConnection(${signature})`,
    operationKind: "method" as const,
    target: { form: "call" as const, path: `node_net::${path}`, argModes },
    resultCarrier: netSocketCarrier,
    parameterCarriers,
    ...providerNativeFallibility,
  }));
}
