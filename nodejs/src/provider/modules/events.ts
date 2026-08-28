import {
  boolCarrier,
  booleanType,
  constructorMember,
  emptyCallbackCarrier,
  eventEmitterCarrier,
  fnExport,
  int32Carrier,
  jsValueCarrier,
  methodMember,
  mutableEventEmitterCarrier,
  numberType,
  oneValueCallbackCarrier,
  providerNativeFallibility,
  providerRef,
  rustJsArrayTargetType,
  rustUsizeToInt32ValueConversion,
  stringType,
  threeValueCallbackCarrier,
  twoValueCallbackCarrier,
  voidType,
} from "../model.js";
import type {
  ProviderTypeExpr,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";

const moduleSpecifier = "node:events";
const emitterId = `${moduleSpecifier}::EventEmitter`;
const anyType = { kind: "any" } as const;
const symbolType = { kind: "source-global", name: "Symbol" } as const;
const eventNameType: ProviderTypeExpr = {
  kind: "union",
  types: [stringType, symbolType],
};
const eventNameArrayType: ProviderTypeExpr = {
  kind: "array",
  elementType: eventNameType,
};
const eventNameArrayCarrier = rustJsArrayTargetType(jsValueCarrier);
const listenerType = (arity: number): ProviderTypeExpr => ({
  kind: "function",
  id: `${moduleSpecifier}.Listener${arity}`,
  parameters: Array.from({ length: arity }, (_, index) => ({
    name: `value${index}`,
    type: anyType,
  })),
  returnType: voidType,
});

const listenerMethod = (memberName:
  | "addListener"
  | "on"
  | "once"
  | "off"
  | "prependListener"
  | "prependOnceListener"
  | "removeListener") => ({
  id: `${emitterId}.${memberName}`,
  name: memberName,
  kind: "method" as const,
  signatures: Array.from({ length: 4 }, (_, arity) => ({
    id: `${emitterId}.${memberName}(${arity})`,
    parameters: [
      { name: "eventName", type: eventNameType },
      { name: "listener", type: listenerType(arity) },
    ],
    returnType: providerRef(moduleSpecifier, "EventEmitter"),
  })),
});

const emitMethod = {
  id: `${emitterId}.emit`,
  name: "emit",
  kind: "method" as const,
  signatures: Array.from({ length: 4 }, (_, arity) => ({
    id: `${emitterId}.emit(${arity})`,
    parameters: [
      { name: "eventName", type: eventNameType },
      ...Array.from({ length: arity }, (__, index) => ({
        name: `value${index}`,
        type: anyType,
      })),
    ],
    returnType: booleanType,
  })),
};

export function eventsModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.events",
    exports: [
      {
        id: emitterId,
        name: "EventEmitter",
        kind: "class",
        members: [
          constructorMember(emitterId, []),
          listenerMethod("addListener"),
          listenerMethod("on"),
          listenerMethod("once"),
          listenerMethod("off"),
          listenerMethod("prependListener"),
          listenerMethod("prependOnceListener"),
          listenerMethod("removeListener"),
          emitMethod,
          methodMember(emitterId, "eventNames", [], eventNameArrayType),
          methodMember(emitterId, "getMaxListeners", [], numberType),
          methodMember(emitterId, "listenerCount", [{ name: "eventName", type: eventNameType }], numberType),
          {
            id: `${emitterId}.removeAllListeners`,
            name: "removeAllListeners",
            kind: "method",
            signatures: [
              {
                id: `${emitterId}.removeAllListeners()`,
                parameters: [],
                returnType: providerRef(moduleSpecifier, "EventEmitter"),
              },
              {
                id: `${emitterId}.removeAllListeners(eventName)`,
                parameters: [{ name: "eventName", type: eventNameType }],
                returnType: providerRef(moduleSpecifier, "EventEmitter"),
              },
            ],
          },
          methodMember(emitterId, "setMaxListeners", [{ name: "count", type: numberType }], providerRef(moduleSpecifier, "EventEmitter")),
        ],
      },
      fnExport(moduleSpecifier, "listenerCount", [
        { name: "emitter", type: providerRef(moduleSpecifier, "EventEmitter") },
        { name: "eventName", type: eventNameType },
      ], numberType),
    ],
  };
}

export function eventsRows(): readonly RustProviderOperationDefinition[] {
  const callbackCarriers = [
    emptyCallbackCarrier,
    oneValueCallbackCarrier,
    twoValueCallbackCarrier,
    threeValueCallbackCarrier,
  ] as const;
  const mutating = (
    memberName:
      | "addListener"
      | "on"
      | "once"
      | "off"
      | "prependListener"
      | "prependOnceListener"
      | "removeListener",
    targetName:
      | "on_callable"
      | "once_callable"
      | "off_callable"
      | "prepend_callable"
      | "prepend_once_callable",
  ): readonly RustProviderOperationDefinition[] => callbackCarriers.map((callbackCarrier, arity) => ({
      exportId: emitterId,
      memberId: `${emitterId}.${memberName}`,
      signatureId: `${emitterId}.${memberName}(${arity})`,
      operationKind: "method",
      target: {
        form: "receiver-method",
        name: `${targetName}${arity === 0 ? "" : arity}`,
        argModes: ["ref", "ref"],
        mutatesReceiver: true,
      },
      resultCarrier: mutableEventEmitterCarrier,
      receiverCarrier: eventEmitterCarrier,
      parameterCarriers: [jsValueCarrier, callbackCarrier],
      ...providerNativeFallibility,
    }));
  return [
    {
      exportId: emitterId,
      memberId: `${emitterId}.constructor`,
      operationKind: "constructor",
      target: { form: "call", path: "node_events::EventEmitter::new" },
      resultCarrier: eventEmitterCarrier,
      parameterCarriers: [],
    },
    ...mutating("addListener", "on_callable"),
    ...mutating("on", "on_callable"),
    ...mutating("once", "once_callable"),
    ...mutating("off", "off_callable"),
    ...mutating("prependListener", "prepend_callable"),
    ...mutating("prependOnceListener", "prepend_once_callable"),
    ...mutating("removeListener", "off_callable"),
    ...Array.from({ length: 4 }, (_, arity): RustProviderOperationDefinition => ({
        exportId: emitterId,
        memberId: `${emitterId}.emit`,
        signatureId: `${emitterId}.emit(${arity})`,
        operationKind: "method",
        target: {
          form: "receiver-method",
          name: `emit_callable${arity === 0 ? "" : arity}`,
          argModes: ["ref", ...Array.from({ length: arity }, () => "value" as const)],
          mutatesReceiver: true,
        },
        resultCarrier: boolCarrier,
        receiverCarrier: eventEmitterCarrier,
        parameterCarriers: [
          jsValueCarrier,
          ...Array.from({ length: arity }, () => jsValueCarrier),
        ],
        ...providerNativeFallibility,
      })),
    {
      exportId: emitterId,
      memberId: `${emitterId}.listenerCount`,
      operationKind: "method",
      target: { form: "receiver-method", name: "callable_listener_count", argModes: ["ref"] },
      resultCarrier: int32Carrier,
      receiverCarrier: eventEmitterCarrier,
      parameterCarriers: [jsValueCarrier],
      resultConversion: rustUsizeToInt32ValueConversion,
      ...providerNativeFallibility,
    },
    {
      exportId: emitterId,
      memberId: `${emitterId}.removeAllListeners`,
      operationKind: "method",
      target: { form: "receiver-method", name: "remove_all_callable_listeners", mutatesReceiver: true },
      resultCarrier: mutableEventEmitterCarrier,
      receiverCarrier: eventEmitterCarrier,
      parameterCarriers: [],
    },
    {
      exportId: emitterId,
      memberId: `${emitterId}.removeAllListeners`,
      signatureId: `${emitterId}.removeAllListeners(eventName)`,
      operationKind: "method",
      target: {
        form: "receiver-method",
        name: "remove_all_callable_listeners_for",
        argModes: ["ref"],
        mutatesReceiver: true,
      },
      resultCarrier: mutableEventEmitterCarrier,
      receiverCarrier: eventEmitterCarrier,
      parameterCarriers: [jsValueCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: emitterId,
      memberId: `${emitterId}.eventNames`,
      operationKind: "method",
      target: { form: "receiver-method", name: "callable_event_names" },
      resultCarrier: eventNameArrayCarrier,
      receiverCarrier: eventEmitterCarrier,
      parameterCarriers: [],
    },
    {
      exportId: emitterId,
      memberId: `${emitterId}.getMaxListeners`,
      operationKind: "method",
      target: { form: "receiver-method", name: "get_max_listeners" },
      resultCarrier: int32Carrier,
      receiverCarrier: eventEmitterCarrier,
      parameterCarriers: [],
      resultConversion: rustUsizeToInt32ValueConversion,
    },
    {
      exportId: emitterId,
      memberId: `${emitterId}.setMaxListeners`,
      operationKind: "method",
      target: { form: "receiver-method", name: "set_max_listeners_i32", mutatesReceiver: true },
      resultCarrier: mutableEventEmitterCarrier,
      receiverCarrier: eventEmitterCarrier,
      parameterCarriers: [int32Carrier],
      ...providerNativeFallibility,
    },
    {
      exportId: `${moduleSpecifier}::listenerCount`,
      operationKind: "method",
      target: { form: "call", path: "node_events::listener_count_callable", argModes: ["ref", "ref"] },
      resultCarrier: int32Carrier,
      parameterCarriers: [eventEmitterCarrier, jsValueCarrier],
      resultConversion: rustUsizeToInt32ValueConversion,
      ...providerNativeFallibility,
    },
  ];
}
