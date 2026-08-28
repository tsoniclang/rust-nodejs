import {
  boolCarrier,
  booleanType,
  bufferCarrier,
  constructorMember,
  methodMember,
  providerNativeFallibility,
  providerRef,
  readableCarrier,
  rustOptionTargetType,
  undefinedType,
  unitCarrier,
  writableCarrier,
  voidType,
} from "../model.js";
import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustTargetTypeRef,
} from "../model.js";

const moduleSpecifier = "node:stream";
const readableId = `${moduleSpecifier}::Readable`;
const writableId = `${moduleSpecifier}::Writable`;
const bufferType = providerRef("node:buffer", "Buffer");
const optionalBufferType = { kind: "union", types: [bufferType, undefinedType] } as const;
const mutableWritableCarrier: RustTargetTypeRef = {
  kind: "reference",
  referent: writableCarrier,
  mutable: true,
};
const mutableReadableCarrier: RustTargetTypeRef = {
  kind: "reference",
  referent: readableCarrier,
  mutable: true,
};

export function streamModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.stream",
    imports: [{ moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] }],
    exports: [
      {
        id: readableId,
        name: "Readable",
        kind: "class",
        members: [
          constructorMember(readableId, []),
          methodMember(readableId, "read", [], optionalBufferType),
          methodMember(readableId, "pipe", [{ name: "destination", type: providerRef(moduleSpecifier, "Writable") }], providerRef(moduleSpecifier, "Writable")),
          methodMember(readableId, "pause", [], providerRef(moduleSpecifier, "Readable")),
          methodMember(readableId, "resume", [], providerRef(moduleSpecifier, "Readable")),
          methodMember(readableId, "isPaused", [], booleanType),
        ],
      },
      {
        id: writableId,
        name: "Writable",
        kind: "class",
        members: [
          constructorMember(writableId, []),
          methodMember(writableId, "write", [{ name: "chunk", type: bufferType }], booleanType),
          methodMember(writableId, "end", [], voidType),
          methodMember(writableId, "cork", [], voidType),
          methodMember(writableId, "uncork", [], voidType),
        ],
      },
    ],
  };
}

export function streamRows(): readonly RustProviderOperationDefinition[] {
  return [
    {
      exportId: readableId,
      memberId: `${readableId}.constructor`,
      operationKind: "constructor",
      target: { form: "call", path: "node_stream::Readable::default" },
      resultCarrier: readableCarrier,
      parameterCarriers: [],
    },
    {
      exportId: readableId,
      memberId: `${readableId}.read`,
      operationKind: "method",
      target: { form: "receiver-method", name: "read", mutatesReceiver: true },
      resultCarrier: rustOptionTargetType(bufferCarrier),
      receiverCarrier: readableCarrier,
      parameterCarriers: [],
    },
    {
      exportId: readableId,
      memberId: `${readableId}.pipe`,
      operationKind: "method",
      target: { form: "receiver-method", name: "pipe_to", argModes: ["mut-ref"], mutatesReceiver: true },
      resultCarrier: mutableWritableCarrier,
      receiverCarrier: readableCarrier,
      parameterCarriers: [writableCarrier],
      ...providerNativeFallibility,
    },
    ...(["pause", "resume"] as const).map((name): RustProviderOperationDefinition => ({
      exportId: readableId,
      memberId: `${readableId}.${name}`,
      operationKind: "method",
      target: { form: "receiver-method", name: `${name}_chain`, mutatesReceiver: true },
      resultCarrier: mutableReadableCarrier,
      receiverCarrier: readableCarrier,
      parameterCarriers: [],
    })),
    {
      exportId: readableId,
      memberId: `${readableId}.isPaused`,
      operationKind: "method",
      target: { form: "receiver-method", name: "is_paused" },
      resultCarrier: boolCarrier,
      receiverCarrier: readableCarrier,
      parameterCarriers: [],
    },
    {
      exportId: writableId,
      memberId: `${writableId}.constructor`,
      operationKind: "constructor",
      target: { form: "call", path: "node_stream::Writable::new" },
      resultCarrier: writableCarrier,
      parameterCarriers: [],
    },
    {
      exportId: writableId,
      memberId: `${writableId}.write`,
      operationKind: "method",
      target: { form: "receiver-method", name: "write", argModes: ["value"], mutatesReceiver: true },
      resultCarrier: boolCarrier,
      receiverCarrier: writableCarrier,
      parameterCarriers: [bufferCarrier],
    },
    ...(["end", "cork", "uncork"] as const).map((name): RustProviderOperationDefinition => ({
      exportId: writableId,
      memberId: `${writableId}.${name}`,
      operationKind: "method",
      target: { form: "receiver-method", name, mutatesReceiver: true },
      resultCarrier: unitCarrier,
      receiverCarrier: writableCarrier,
      parameterCarriers: [],
    })),
  ];
}
