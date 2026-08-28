import {
  boolCarrier,
  booleanType,
  bufferCarrier,
  httpServerResponseCarrier,
  methodMember,
  providerNativeFallibility,
  providerRef,
  readableCarrier,
  rustOptionTargetType,
  stringCarrier,
  stringType,
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
const serverResponseType = providerRef("node:http", "ServerResponse");
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
    imports: [
      { moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] },
      { moduleSpecifier: "node:http", namedImports: [{ exportedName: "ServerResponse" }] },
    ],
    exports: [
      {
        id: readableId,
        name: "Readable",
        kind: "class",
        members: [
          methodMember(readableId, "read", [], optionalBufferType),
          {
            id: `${readableId}.pipe`,
            name: "pipe",
            kind: "method",
            signatures: [
              {
                id: `${readableId}.pipe(writable)`,
                parameters: [{ name: "destination", type: providerRef(moduleSpecifier, "Writable") }],
                returnType: providerRef(moduleSpecifier, "Writable"),
              },
              {
                id: `${readableId}.pipe(serverResponse)`,
                parameters: [{ name: "destination", type: serverResponseType }],
                returnType: serverResponseType,
              },
            ],
          },
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
          {
            id: `${writableId}.write`,
            name: "write",
            kind: "method",
            signatures: [
              { id: `${writableId}.write(buffer)`, parameters: [{ name: "chunk", type: bufferType }], returnType: booleanType },
              { id: `${writableId}.write(string)`, parameters: [{ name: "chunk", type: stringType }], returnType: booleanType },
            ],
          },
          {
            id: `${writableId}.end`,
            name: "end",
            kind: "method",
            signatures: [
              { id: `${writableId}.end()`, parameters: [], returnType: providerRef(moduleSpecifier, "Writable") },
              { id: `${writableId}.end(buffer)`, parameters: [{ name: "chunk", type: bufferType }], returnType: providerRef(moduleSpecifier, "Writable") },
              { id: `${writableId}.end(string)`, parameters: [{ name: "chunk", type: stringType }], returnType: providerRef(moduleSpecifier, "Writable") },
            ],
          },
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
      signatureId: `${readableId}.pipe(writable)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "pipe_to", argModes: ["mut-ref"], mutatesReceiver: true },
      resultCarrier: mutableWritableCarrier,
      receiverCarrier: readableCarrier,
      parameterCarriers: [writableCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: readableId,
      memberId: `${readableId}.pipe`,
      signatureId: `${readableId}.pipe(serverResponse)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "pipe_to", argModes: ["mut-ref"], mutatesReceiver: true },
      resultCarrier: { kind: "reference", referent: httpServerResponseCarrier, mutable: true },
      receiverCarrier: readableCarrier,
      parameterCarriers: [httpServerResponseCarrier],
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
      memberId: `${writableId}.write`,
      signatureId: `${writableId}.write(buffer)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "write_buffer", argModes: ["ref"], mutatesReceiver: true },
      resultCarrier: boolCarrier,
      receiverCarrier: writableCarrier,
      parameterCarriers: [bufferCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: writableId,
      memberId: `${writableId}.write`,
      signatureId: `${writableId}.write(string)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "write_string", argModes: ["ref"], mutatesReceiver: true },
      resultCarrier: boolCarrier,
      receiverCarrier: writableCarrier,
      parameterCarriers: [stringCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: writableId,
      memberId: `${writableId}.end`,
      signatureId: `${writableId}.end()`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end", mutatesReceiver: true },
      resultCarrier: mutableWritableCarrier,
      receiverCarrier: writableCarrier,
      parameterCarriers: [],
    },
    {
      exportId: writableId,
      memberId: `${writableId}.end`,
      signatureId: `${writableId}.end(buffer)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end_buffer", argModes: ["ref"], mutatesReceiver: true },
      resultCarrier: mutableWritableCarrier,
      receiverCarrier: writableCarrier,
      parameterCarriers: [bufferCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: writableId,
      memberId: `${writableId}.end`,
      signatureId: `${writableId}.end(string)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end_string", argModes: ["ref"], mutatesReceiver: true },
      resultCarrier: mutableWritableCarrier,
      receiverCarrier: writableCarrier,
      parameterCarriers: [stringCarrier],
      ...providerNativeFallibility,
    },
    ...(["cork", "uncork"] as const).map((name): RustProviderOperationDefinition => ({
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
