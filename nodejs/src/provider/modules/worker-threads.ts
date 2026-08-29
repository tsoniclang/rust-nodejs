import {
  boolCarrier,
  booleanType,
  cloneDefaultCarrierTraits,
  cloneOnlyCarrierTraits,
  emptyCallbackCarrier,
  int32Carrier,
  jsValueCarrier,
  messageChannelCarrier,
  messagePortCarrier,
  numberType,
  oneValueCallbackCarrier,
  propertyMember,
  providerNativeFallibility,
  providerRef,
  rustJsPromiseTargetType,
  rustOptionTargetType,
  stringArrayCarrier,
  stringArrayType,
  stringCarrier,
  stringType,
  unitCarrier,
  undefinedType,
  valueExport,
  voidType,
  workerCarrier,
  workerOptionsCarrier,
} from "../model.js";
import type {
  ProviderTypeExpr,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustTargetTypeRef,
} from "../model.js";

const moduleSpecifier = "node:worker_threads";
const workerId = `${moduleSpecifier}::Worker`;
const workerOptionsId = `${moduleSpecifier}::WorkerOptions`;
const messagePortId = `${moduleSpecifier}::MessagePort`;
const messageChannelId = `${moduleSpecifier}::MessageChannel`;
const anyType = { kind: "any" } as const;
const optionalUnknownType = { kind: "union", types: [anyType, undefinedType] } as const;
const optionalPortType = {
  kind: "union",
  types: [providerRef(moduleSpecifier, "MessagePort"), undefinedType],
} as const;
const promiseNumberType = {
  kind: "source-global",
  name: "Promise",
  typeArguments: [numberType],
} as const;
const callbackType = (arity: number): ProviderTypeExpr => ({
  kind: "function",
  id: `${moduleSpecifier}.Listener${arity}`,
  parameters: Array.from({ length: arity }, (_, index) => ({
    name: `value${index}`,
    type: anyType,
  })),
  returnType: voidType,
});

export function workerThreadsModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.worker-threads",
    exports: [
      {
        id: workerId,
        name: "Worker",
        kind: "class",
        members: [
          {
            id: `${workerId}.constructor`,
            name: "constructor",
            kind: "constructor",
            signatures: [
              {
                id: `${workerId}.constructor(modulePath)`,
                parameters: [{ name: "modulePath", type: stringType }],
                returnType: voidType,
              },
              {
                id: `${workerId}.constructor(modulePath,options)`,
                parameters: [
                  { name: "modulePath", type: stringType },
                  { name: "options", type: providerRef(moduleSpecifier, "WorkerOptions") },
                ],
                returnType: voidType,
              },
            ],
          },
          method(workerId, "postMessage", [{ name: "value", type: anyType }], voidType),
          method(workerId, "terminate", [], promiseNumberType),
          method(workerId, "ref", [], providerRef(moduleSpecifier, "Worker")),
          method(workerId, "unref", [], providerRef(moduleSpecifier, "Worker")),
          ...eventMembers(workerId),
          propertyMember(workerId, "threadId", numberType),
        ],
      },
      {
        id: workerOptionsId,
        name: "WorkerOptions",
        kind: "interface",
        members: [
          propertyMember(workerOptionsId, "name", stringType, { readonly: false, optional: true }),
          propertyMember(workerOptionsId, "argv", stringArrayType, { readonly: false, optional: true }),
          propertyMember(workerOptionsId, "env", anyType, { readonly: false, optional: true }),
          propertyMember(workerOptionsId, "workerData", anyType, { readonly: false, optional: true }),
        ],
      },
      {
        id: messagePortId,
        name: "MessagePort",
        kind: "class",
        members: [
          method(messagePortId, "postMessage", [{ name: "value", type: anyType }], voidType),
          method(messagePortId, "start", [], voidType),
          method(messagePortId, "close", [], voidType),
          method(messagePortId, "ref", [], providerRef(moduleSpecifier, "MessagePort")),
          method(messagePortId, "unref", [], providerRef(moduleSpecifier, "MessagePort")),
          method(messagePortId, "hasRef", [], booleanType),
          ...eventMembers(messagePortId),
        ],
      },
      {
        id: messageChannelId,
        name: "MessageChannel",
        kind: "class",
        members: [
          {
            id: `${messageChannelId}.constructor`,
            name: "constructor",
            kind: "constructor",
            signatures: [{
              id: `${messageChannelId}.constructor()`,
              parameters: [],
              returnType: voidType,
            }],
          },
          propertyMember(messageChannelId, "port1", providerRef(moduleSpecifier, "MessagePort")),
          propertyMember(messageChannelId, "port2", providerRef(moduleSpecifier, "MessagePort")),
        ],
      },
      functionExport("receiveMessageOnPort", [
        { name: "port", type: providerRef(moduleSpecifier, "MessagePort") },
      ], optionalUnknownType),
      functionExport("getEnvironmentData", [{ name: "key", type: stringType }], optionalUnknownType),
      functionExport("setEnvironmentData", [
        { name: "key", type: stringType },
        { name: "value", type: anyType },
      ], voidType),
      functionExport("markAsUntransferable", [{ name: "value", type: anyType }], voidType),
      functionExport("isMarkedAsUntransferable", [{ name: "value", type: anyType }], booleanType),
      valueExport(moduleSpecifier, "isMainThread", booleanType),
      valueExport(moduleSpecifier, "threadId", numberType),
      valueExport(moduleSpecifier, "workerData", anyType),
      valueExport(moduleSpecifier, "parentPort", optionalPortType),
    ],
  };
}

export function workerThreadsRows(): readonly RustProviderOperationDefinition[] {
  const bootstrap = {
    id: "tsonic.rust.node.worker-threads.process-v1",
    path: "node_worker_threads::initialize_worker_process",
    errorBoundary: "provider-native" as const,
    errorCarrier: { kind: "target-named", id: "rust.node.NodeError" } as const,
  };
  return [
    {
      exportId: workerId,
      memberId: `${workerId}.constructor`,
      signatureId: `${workerId}.constructor(modulePath)`,
      operationKind: "constructor",
      target: {
        form: "source-module-construction",
        path: "node_worker_threads::Worker::spawn_default",
        sourceArgumentIndex: 0,
        targetArgumentIndex: 0,
        bootstrap,
        argModes: ["ref"],
      },
      resultCarrier: workerCarrier,
      parameterCarriers: [stringCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: workerId,
      memberId: `${workerId}.constructor`,
      signatureId: `${workerId}.constructor(modulePath,options)`,
      operationKind: "constructor",
      target: {
        form: "source-module-construction",
        path: "node_worker_threads::Worker::spawn_with_options",
        sourceArgumentIndex: 0,
        targetArgumentIndex: 0,
        bootstrap,
        argModes: ["ref", "value"],
      },
      resultCarrier: workerCarrier,
      parameterCarriers: [stringCarrier, workerOptionsCarrier],
      ...providerNativeFallibility,
    },
    receiverMethod(workerId, "postMessage", "post_message", workerCarrier, [jsValueCarrier], unitCarrier, false, true),
    {
      ...receiverMethod(
        workerId,
        "terminate",
        "terminate",
        workerCarrier,
        [],
        rustJsPromiseTargetType(int32Carrier),
        true,
      ),
      ...providerNativeFallibility,
    },
    receiverMethod(workerId, "ref", "ref_chain", workerCarrier, [], mutableReference(workerCarrier), true),
    receiverMethod(workerId, "unref", "unref", workerCarrier, [], mutableReference(workerCarrier), true),
    ...eventRows(workerId, workerCarrier),
    {
      exportId: workerId,
      memberId: `${workerId}.threadId`,
      operationKind: "property",
      target: { form: "receiver-method", name: "thread_id" },
      resultCarrier: int32Carrier,
      receiverCarrier: workerCarrier,
    },
    {
      exportId: messageChannelId,
      memberId: `${messageChannelId}.constructor`,
      operationKind: "constructor",
      target: { form: "call", path: "node_worker_threads::MessageChannel::new" },
      resultCarrier: messageChannelCarrier,
      parameterCarriers: [],
    },
    ...(["port1", "port2"] as const).map((name): RustProviderOperationDefinition => ({
      exportId: messageChannelId,
      memberId: `${messageChannelId}.${name}`,
      operationKind: "property",
      target: { form: "field", name },
      resultCarrier: messagePortCarrier,
      receiverCarrier: messageChannelCarrier,
    })),
    receiverMethod(messagePortId, "postMessage", "post_message", messagePortCarrier, [jsValueCarrier], unitCarrier, false, true),
    receiverMethod(messagePortId, "start", "start", messagePortCarrier, [], unitCarrier),
    receiverMethod(messagePortId, "close", "close", messagePortCarrier, [], unitCarrier),
    receiverMethod(messagePortId, "ref", "ref_chain", messagePortCarrier, [], mutableReference(messagePortCarrier), true),
    receiverMethod(messagePortId, "unref", "unref", messagePortCarrier, [], mutableReference(messagePortCarrier), true),
    receiverMethod(messagePortId, "hasRef", "has_ref", messagePortCarrier, [], boolCarrier),
    ...eventRows(messagePortId, messagePortCarrier),
    moduleCall("receiveMessageOnPort", "receive_message_on_port", [messagePortCarrier], jsValueCarrier, ["ref"]),
    moduleCall("getEnvironmentData", "get_environment_data", [stringCarrier], jsValueCarrier, ["ref"]),
    {
      ...moduleCall("setEnvironmentData", "set_environment_data", [stringCarrier, jsValueCarrier], unitCarrier, ["ref", "value"]),
      ...providerNativeFallibility,
    },
    {
      ...moduleCall("markAsUntransferable", "mark_as_untransferable", [jsValueCarrier], unitCarrier, ["ref"]),
      ...providerNativeFallibility,
    },
    {
      ...moduleCall("isMarkedAsUntransferable", "is_marked_as_untransferable", [jsValueCarrier], boolCarrier, ["ref"]),
      ...providerNativeFallibility,
    },
    moduleProperty("isMainThread", "is_main_thread", boolCarrier),
    moduleProperty("threadId", "thread_id", int32Carrier),
    moduleProperty("workerData", "worker_data", jsValueCarrier),
    moduleProperty("parentPort", "parent_port", rustOptionTargetType(messagePortCarrier)),
    ...optionRows(workerOptionsId, workerOptionsCarrier, [
      ["name", "name", rustOptionTargetType(stringCarrier)],
      ["argv", "argv", rustOptionTargetType(stringArrayCarrier)],
      ["env", "env", jsValueCarrier],
      ["workerData", "worker_data", jsValueCarrier],
    ]),
  ];
}

export const workerThreadCarrierTraits = {
  worker: cloneOnlyCarrierTraits,
  options: cloneDefaultCarrierTraits,
  port: cloneOnlyCarrierTraits,
};

function method(
  owner: string,
  name: string,
  parameters: readonly { readonly name: string; readonly type: ProviderTypeExpr }[],
  returnType: ProviderTypeExpr,
) {
  return {
    id: `${owner}.${name}`,
    name,
    kind: "method" as const,
    signatures: [{
      id: `${owner}.${name}(${parameters.map((parameter) => parameter.name).join(",")})`,
      parameters,
      returnType,
    }],
  };
}

function eventMembers(owner: string) {
  return (["on", "once", "off"] as const).map((name) => ({
    id: `${owner}.${name}`,
    name,
    kind: "method" as const,
    signatures: [0, 1].map((arity) => ({
      id: `${owner}.${name}(${arity})`,
      parameters: [
        { name: "eventName", type: stringType },
        { name: "listener", type: callbackType(arity) },
      ],
      returnType: providerRef(moduleSpecifier, owner === workerId ? "Worker" : "MessagePort"),
    })),
  }));
}

function eventRows(
  owner: string,
  receiverCarrier: RustTargetTypeRef,
): readonly RustProviderOperationDefinition[] {
  return (["on", "once", "off"] as const).flatMap((name) => [
    eventRow(owner, name, 0, receiverCarrier, emptyCallbackCarrier),
    eventRow(owner, name, 1, receiverCarrier, oneValueCallbackCarrier),
  ]);
}

function eventRow(
  owner: string,
  name: "on" | "once" | "off",
  arity: 0 | 1,
  receiverCarrier: RustTargetTypeRef,
  callbackCarrier: RustTargetTypeRef,
): RustProviderOperationDefinition {
  return {
    exportId: owner,
    memberId: `${owner}.${name}`,
    signatureId: `${owner}.${name}(${arity})`,
    operationKind: "method",
    target: {
      form: "receiver-method",
      name: `${name === "on" ? "on" : name === "once" ? "once" : "off"}_callable${arity === 0 ? "" : "1"}`,
      argModes: ["ref", "ref"],
      mutatesReceiver: true,
    },
    resultCarrier: mutableReference(receiverCarrier),
    receiverCarrier,
    parameterCarriers: [jsValueCarrier, callbackCarrier],
    ...providerNativeFallibility,
  };
}

function receiverMethod(
  exportId: string,
  memberName: string,
  targetName: string,
  receiverCarrier: RustTargetTypeRef,
  parameterCarriers: readonly RustTargetTypeRef[],
  resultCarrier: RustTargetTypeRef,
  mutatesReceiver = false,
  fallible = false,
): RustProviderOperationDefinition {
  return {
    exportId,
    memberId: `${exportId}.${memberName}`,
    operationKind: "method",
    target: {
      form: "receiver-method",
      name: targetName,
      ...(mutatesReceiver ? { mutatesReceiver: true } : {}),
    },
    resultCarrier,
    receiverCarrier,
    parameterCarriers,
    ...(fallible ? providerNativeFallibility : {}),
  } as RustProviderOperationDefinition;
}

function functionExport(
  name: string,
  parameters: readonly { readonly name: string; readonly type: ProviderTypeExpr }[],
  returnType: ProviderTypeExpr,
) {
  return {
    id: `${moduleSpecifier}::${name}`,
    name,
    kind: "function" as const,
    signatures: [{
      id: `${moduleSpecifier}::${name}(${parameters.map((parameter) => parameter.name).join(",")})`,
      parameters,
      returnType,
    }],
  };
}

function moduleCall(
  name: string,
  targetName: string,
  parameterCarriers: readonly RustTargetTypeRef[],
  resultCarrier: RustTargetTypeRef,
  argModes?: readonly ("value" | "ref" | "mut-ref")[],
): RustProviderOperationDefinition {
  return {
    exportId: `${moduleSpecifier}::${name}`,
    operationKind: "method",
    target: {
      form: "call",
      path: `node_worker_threads::${targetName}`,
      ...(argModes === undefined ? {} : { argModes }),
    },
    resultCarrier,
    parameterCarriers,
  };
}

function moduleProperty(
  name: string,
  targetName: string,
  resultCarrier: RustTargetTypeRef,
): RustProviderOperationDefinition {
  return {
    exportId: `${moduleSpecifier}::${name}`,
    operationKind: "property",
    target: { form: "call", path: `node_worker_threads::${targetName}` },
    resultCarrier,
  };
}

function optionRows(
  exportId: string,
  receiverCarrier: RustTargetTypeRef,
  fields: readonly (readonly [string, string, RustTargetTypeRef])[],
): readonly RustProviderOperationDefinition[] {
  return fields.flatMap(([sourceName, targetName, carrier]) => [{
    exportId,
    memberId: `${exportId}.${sourceName}`,
    operationKind: "property" as const,
    target: { form: "field" as const, name: targetName },
    resultCarrier: carrier,
    receiverCarrier,
  }, {
    exportId,
    memberId: `${exportId}.${sourceName}`,
    operationKind: "property-set" as const,
    target: { form: "field" as const, name: targetName },
    resultCarrier: unitCarrier,
    parameterCarriers: [carrier],
    receiverCarrier,
  }]);
}

function mutableReference(referent: RustTargetTypeRef): RustTargetTypeRef {
  return { kind: "reference", referent, mutable: true };
}
