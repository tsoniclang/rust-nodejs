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
import {
  duplexCarrier,
  readableCarrier,
  streamCarrier,
  transformCarrier,
  writableCarrier,
} from "./carriers.js";

const moduleSpecifier = "node:stream";
const streamId = `${moduleSpecifier}::Stream`;
const readableId = `${moduleSpecifier}::Readable`;
const writableId = `${moduleSpecifier}::Writable`;
const duplexId = `${moduleSpecifier}::Duplex`;
const transformId = `${moduleSpecifier}::Transform`;
const bufferType = providerRef("node:buffer", "Buffer");
const errorType = { kind: "source-global", name: "Error" } as const;
const optionalBufferType = { kind: "union", types: [bufferType, undefinedType] } as const;
const bufferArrayType = { kind: "array", elementType: bufferType } as const;
const emptyListenerCarrier = rustCallableTargetType([], unitCarrier);
const dataListenerCarrier = rustCallableTargetType([bufferCarrier], unitCarrier);
const errorListenerCarrier = rustCallableTargetType([nodeErrorCarrier], unitCarrier);
const destinationCarrier: RustTargetTypeRef = { kind: "type-parameter", name: "TDestination" };
const optionalErrorCarrier = rustOptionTargetType(nodeErrorCarrier);

export function streamModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.stream",
    imports: [
      { moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] },
    ],
    exports: [
      {
        id: streamId,
        name: "Stream",
        kind: "class",
        members: [],
      },
      readableDeclaration(),
      writableDeclaration(),
      duplexDeclaration(),
      {
        id: transformId,
        name: "Transform",
        kind: "class",
        heritage: [{ kind: "extends", type: providerRef(moduleSpecifier, "Duplex") }],
        members: [],
      },
    ],
  };
}

function readableDeclaration(): RustProviderModuleDefinition["exports"][number] {
  return {
    id: readableId,
    name: "Readable",
    kind: "class",
    heritage: [{ kind: "extends", type: providerRef(moduleSpecifier, "Stream") }],
    members: [
      {
        id: `${readableId}.from`,
        name: "from",
        kind: "method",
        static: true,
        signatures: [{
          id: `${readableId}.from(chunks)`,
          parameters: [{ name: "chunks", type: bufferArrayType }],
          returnType: providerRef(moduleSpecifier, "Readable"),
        }],
      },
      {
        id: `${readableId}.read`,
        name: "read",
        kind: "method",
        signatures: [{
          id: `${readableId}.read(size)`,
          parameters: [{ name: "size", type: int32Type, optional: true }],
          returnType: optionalBufferType,
        }],
      },
      {
        id: `${readableId}.pipe`,
        name: "pipe",
        kind: "method",
        signatures: [{
          id: `${readableId}.pipe(destination)`,
          typeParameters: [{
            name: "TDestination",
            constraints: [providerRef(moduleSpecifier, "Writable")],
          }],
          parameters: [{
            name: "destination",
            type: { kind: "type-parameter", name: "TDestination" },
          }],
          returnType: { kind: "type-parameter", name: "TDestination" },
        }],
      },
      method(readableId, "pause", [], providerRef(moduleSpecifier, "Readable")),
      method(readableId, "resume", [], providerRef(moduleSpecifier, "Readable")),
      method(readableId, "isPaused", [], booleanType),
      method(readableId, "destroy", [{ name: "error", type: errorType, optional: true }], providerRef(moduleSpecifier, "Readable")),
      property(readableId, "readable", booleanType),
      property(readableId, "readableEnded", booleanType),
      property(readableId, "destroyed", booleanType),
      ...eventMembers(readableId, providerRef(moduleSpecifier, "Readable"), [
        ["data", [{ name: "chunk", type: bufferType }]],
        ["end", []],
        ["error", [{ name: "error", type: errorType }]],
        ["close", []],
      ]),
    ],
  };
}

function writableDeclaration(): RustProviderModuleDefinition["exports"][number] {
  return {
    id: writableId,
    name: "Writable",
    kind: "class",
    heritage: [{ kind: "extends", type: providerRef(moduleSpecifier, "Stream") }],
    members: writableMembers("Writable"),
  };
}

function duplexDeclaration(): RustProviderModuleDefinition["exports"][number] {
  return {
    id: duplexId,
    name: "Duplex",
    kind: "class",
    heritage: [{ kind: "extends", type: providerRef(moduleSpecifier, "Readable") }],
    members: writableMembers("Duplex"),
  };
}

function writableMembers(className: "Writable" | "Duplex") {
  const classId = className === "Writable" ? writableId : duplexId;
  const classType = providerRef(moduleSpecifier, className);
  return [
    {
      id: `${classId}.write`,
      name: "write",
      kind: "method" as const,
      signatures: [
        { id: `${classId}.write(buffer)`, parameters: [{ name: "chunk", type: bufferType }], returnType: booleanType },
        { id: `${classId}.write(string)`, parameters: [{ name: "chunk", type: stringType }], returnType: booleanType },
      ],
    },
    {
      id: `${classId}.end`,
      name: "end",
      kind: "method" as const,
      signatures: [
        { id: `${classId}.end()`, parameters: [], returnType: classType },
        { id: `${classId}.end(buffer)`, parameters: [{ name: "chunk", type: bufferType }], returnType: classType },
        { id: `${classId}.end(string)`, parameters: [{ name: "chunk", type: stringType }], returnType: classType },
      ],
    },
    method(classId, "cork", [], voidType),
    method(classId, "uncork", [], voidType),
    method(classId, "destroy", [{ name: "error", type: errorType, optional: true }], classType),
    property(classId, "writable", booleanType),
    property(classId, "writableEnded", booleanType),
    property(classId, "writableFinished", booleanType),
    property(classId, "writableNeedDrain", booleanType),
    property(classId, "destroyed", booleanType),
    ...eventMembers(classId, classType, [
      ...(className === "Duplex" ? [
        ["data", [{ name: "chunk", type: bufferType }]],
        ["end", []],
      ] as const : []),
      ["drain", []],
      ["finish", []],
      ["error", [{ name: "error", type: errorType }]],
      ["close", []],
    ]),
  ];
}

function method(
  classId: string,
  name: string,
  parameters: readonly { readonly name: string; readonly type: ProviderTypeExpr; readonly optional?: boolean }[],
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

function property(classId: string, name: string, type: ProviderTypeExpr) {
  return { id: `${classId}.${name}`, name, kind: "property" as const, readonly: true, type };
}

function eventMembers(
  classId: string,
  returnType: ProviderTypeExpr,
  events: readonly (readonly [
    name: string,
    parameters: readonly { readonly name: string; readonly type: ProviderTypeExpr }[],
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

export function streamRows(): readonly RustProviderOperationDefinition[] {
  return [
    {
      exportId: readableId,
      memberId: `${readableId}.from`,
      signatureId: `${readableId}.from(chunks)`,
      operationKind: "method",
      target: { form: "call", path: "node_stream::Readable::from_source", argModes: ["ref"] },
      resultCarrier: readableCarrier,
      parameterCarriers: [rustJsArrayTargetType(bufferCarrier)],
      ...providerNativeFallibility,
    },
    {
      exportId: readableId,
      memberId: `${readableId}.read`,
      signatureId: `${readableId}.read(size)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "read_buffer", argModes: ["value"] },
      resultCarrier: rustOptionTargetType(bufferCarrier),
      receiverCarrier: readableCarrier,
      parameterCarriers: [rustOptionTargetType(int32Carrier)],
      ...providerNativeFallibility,
    },
    {
      exportId: readableId,
      memberId: `${readableId}.pipe`,
      signatureId: `${readableId}.pipe(destination)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "pipe_to", argModes: ["ref"] },
      resultCarrier: destinationCarrier,
      receiverCarrier: readableCarrier,
      parameterCarriers: [destinationCarrier],
      genericParameters: [{ kind: "type", sourceName: "TDestination" }],
      typeRequirements: [{
        name: "TDestination",
        requirements: [
          "clone",
          {
            kind: "trait",
            path: "tsonic_rust_node::stream::WritableTarget",
            genericArguments: [],
            associatedConstraints: [],
          },
        ],
      }],
      ...providerNativeFallibility,
    },
    ...readableSimpleRows(),
    ...writableRows("Writable", writableCarrier),
    ...writableRows("Duplex", duplexCarrier),
  ];
}

function readableSimpleRows(): readonly RustProviderOperationDefinition[] {
  const rows: RustProviderOperationDefinition[] = [
    receiverMethod(readableId, "pause", "pause_chain", readableCarrier, readableCarrier, []),
    receiverMethod(readableId, "resume", "resume_chain", readableCarrier, readableCarrier, []),
    receiverMethod(readableId, "isPaused", "is_paused", readableCarrier, boolCarrier, []),
    {
      ...receiverMethod(readableId, "destroy", "destroy_chain", readableCarrier, readableCarrier, [optionalErrorCarrier]),
      ...providerNativeFallibility,
    },
    receiverProperty(readableId, "readable", readableCarrier, boolCarrier),
    receiverProperty(readableId, "readableEnded", readableCarrier, boolCarrier, "readable_ended"),
    receiverProperty(readableId, "destroyed", readableCarrier, boolCarrier),
  ];
  rows.push(...eventRows(readableId, readableCarrier, [
    ["data", dataListenerCarrier],
    ["end", emptyListenerCarrier],
    ["error", errorListenerCarrier],
    ["close", emptyListenerCarrier],
  ]));
  return rows;
}

function writableRows(
  className: "Writable" | "Duplex",
  receiverCarrier: RustTargetTypeRef,
): readonly RustProviderOperationDefinition[] {
  const classId = className === "Writable" ? writableId : duplexId;
  const rows: RustProviderOperationDefinition[] = [
    {
      ...receiverMethod(classId, "write", "write_buffer", receiverCarrier, boolCarrier, [bufferCarrier], `${classId}.write(buffer)`),
      target: { form: "receiver-method", name: "write_buffer", argModes: ["ref"] },
      ...providerNativeFallibility,
    },
    {
      ...receiverMethod(classId, "write", "write_string", receiverCarrier, boolCarrier, [stringCarrier], `${classId}.write(string)`),
      target: { form: "receiver-method", name: "write_string", argModes: ["ref"] },
      ...providerNativeFallibility,
    },
    receiverMethod(classId, "end", "end", receiverCarrier, receiverCarrier, [], `${classId}.end()`),
    {
      ...receiverMethod(classId, "end", "end_buffer", receiverCarrier, receiverCarrier, [bufferCarrier], `${classId}.end(buffer)`),
      target: { form: "receiver-method", name: "end_buffer", argModes: ["ref"] },
      ...providerNativeFallibility,
    },
    {
      ...receiverMethod(classId, "end", "end_string", receiverCarrier, receiverCarrier, [stringCarrier], `${classId}.end(string)`),
      target: { form: "receiver-method", name: "end_string", argModes: ["ref"] },
      ...providerNativeFallibility,
    },
    receiverMethod(classId, "cork", "cork", receiverCarrier, unitCarrier, []),
    receiverMethod(classId, "uncork", "uncork", receiverCarrier, unitCarrier, []),
    {
      ...receiverMethod(classId, "destroy", "destroy_chain", receiverCarrier, receiverCarrier, [optionalErrorCarrier]),
      ...providerNativeFallibility,
    },
    receiverProperty(classId, "writable", receiverCarrier, boolCarrier),
    receiverProperty(classId, "writableEnded", receiverCarrier, boolCarrier, "writable_ended"),
    receiverProperty(classId, "writableFinished", receiverCarrier, boolCarrier, "writable_finished"),
    receiverProperty(classId, "writableNeedDrain", receiverCarrier, boolCarrier, "writable_need_drain"),
    receiverProperty(classId, "destroyed", receiverCarrier, boolCarrier),
  ];
  rows.push(...eventRows(classId, receiverCarrier, [
    ...(className === "Duplex" ? [
      ["data", dataListenerCarrier],
      ["end", emptyListenerCarrier],
    ] as const : []),
    ["drain", emptyListenerCarrier],
    ["finish", emptyListenerCarrier],
    ["error", errorListenerCarrier],
    ["close", emptyListenerCarrier],
  ]));
  return rows;
}

function receiverMethod(
  exportId: string,
  memberName: string,
  targetName: string,
  receiverCarrier: RustTargetTypeRef,
  resultCarrier: RustTargetTypeRef,
  parameterCarriers: readonly RustTargetTypeRef[],
  signatureId?: string,
): RustProviderOperationDefinition {
  return {
    exportId,
    memberId: `${exportId}.${memberName}`,
    ...(signatureId === undefined ? {} : { signatureId }),
    operationKind: "method",
    target: { form: "receiver-method", name: targetName },
    receiverCarrier,
    resultCarrier,
    parameterCarriers,
  };
}

function receiverProperty(
  exportId: string,
  memberName: string,
  receiverCarrier: RustTargetTypeRef,
  resultCarrier: RustTargetTypeRef,
  targetName = memberName,
): RustProviderOperationDefinition {
  return {
    exportId,
    memberId: `${exportId}.${memberName}`,
    operationKind: "property",
    target: { form: "receiver-method", name: targetName },
    receiverCarrier,
    resultCarrier,
  };
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
