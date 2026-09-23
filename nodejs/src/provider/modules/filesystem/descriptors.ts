import {
  int32Carrier, nativeUintCarrier,
  bufferCarrier, float64Carrier, fnExport, nullType, numberType,
  providerNativeFallibility, providerRef, rustOptionTargetType,
  stringCarrier, stringType, unitCarrier, voidType,
} from "../../model.js";
import { rustJsTypedArrayTargetType } from "@tsonic/target-rust/provider";
import type {
  RustProviderModuleDefinition, RustProviderOperationDefinition,
  RustTargetTypeRef,
} from "../../model.js";

const moduleId = "node:fs";
const positions = [
  { suffix: "", type: numberType, carrier: float64Carrier },
  { suffix: ",int64", type: { kind: "source-primitive", name: "int64" }, carrier: { kind: "source-primitive", name: "int64" } },
] as const;

export function fileDescriptorExports(typedArrays: boolean): RustProviderModuleDefinition["exports"] {
  const buffers = [
    { name: "Buffer", type: providerRef("node:buffer", "Buffer") },
    ...(typedArrays ? [{ name: "Uint8Array", type: { kind: "source-global", name: "Uint8Array" } as const }] : []),
  ];
  return [
    {
      id: `${moduleId}::openSync`, name: "openSync", kind: "function",
      signatures: [
        { id: `${moduleId}::openSync(path,flags)`, name: "openSync", parameters: [
          { name: "path", type: stringType }, { name: "flags", type: stringType },
        ], returnType: numberType },
        ...[
          { name: "string", type: stringType },
          { name: "Buffer", type: providerRef("node:buffer", "Buffer") },
        ].map(path => ({
          id: `${moduleId}::openSync(${path.name},number,number)`, name: "openSync",
          parameters: [{ name: "path", type: path.type }, { name: "flags", type: numberType }, { name: "mode", type: numberType }],
          returnType: numberType,
        })),
      ],
    },
    fnExport(moduleId, "closeSync", [{ name: "fd", type: numberType }], voidType),
    ...["readSync", "writeSync"].map(name => ({
      id: `${moduleId}::${name}`, name, kind: "function" as const,
      signatures: buffers.flatMap(buffer => positions.map(position => ({
        id: `${moduleId}::${name}(${buffer.name}${position.suffix})`, name,
        parameters: [
          { name: "fd", type: numberType }, { name: "buffer", type: buffer.type },
          { name: "offset", type: numberType }, { name: "length", type: numberType },
          { name: "position", type: { kind: "union" as const, types: [position.type, nullType] } },
        ],
        returnType: numberType,
      }))),
    })),
  ];
}

export function fileDescriptorRows(typedArrays: boolean): readonly RustProviderOperationDefinition[] {
  const positionCarrier = { kind: "type-parameter", name: "Position" } as const;
  const numericCarrier = (name: string): RustTargetTypeRef => ({ kind: "type-parameter", name });
  const numericGenerics = (...names: string[]) => names.map(sourceName => ({ kind: "type" as const, sourceName }));
  const operation = (
    name: string, path: string, parameters: readonly RustTargetTypeRef[],
    modes: readonly ("ref" | "value")[], result: RustTargetTypeRef = nativeUintCarrier,
  ): RustProviderOperationDefinition => ({
    exportId: `${moduleId}::${name}`, operationKind: "method",
    target: { form: "call", path: `node_fs::${path}`, argModes: modes },
    resultCarrier: result, parameterCarriers: parameters, ...providerNativeFallibility,
  });
  return [
    { ...operation("openSync", "open_sync", [stringCarrier, stringCarrier], ["ref", "ref"], int32Carrier), signatureId: `${moduleId}::openSync(path,flags)` },
    ...[
      { name: "string", carrier: stringCarrier, path: "open_sync_numeric" },
      { name: "Buffer", carrier: bufferCarrier, path: "open_sync_buffer_numeric" },
    ].map(path => ({
      ...operation("openSync", path.path, [path.carrier, numericCarrier("Flags"), numericCarrier("Mode")], ["ref", "value", "value"], int32Carrier),
      genericParameters: numericGenerics("Flags", "Mode"),
      signatureId: `${moduleId}::openSync(${path.name},number,number)`,
    })),
    { ...operation("closeSync", "close_sync_number", [numericCarrier("Descriptor")], ["value"], unitCarrier),
      genericParameters: numericGenerics("Descriptor") },
    ...[
      { name: "Buffer", carrier: bufferCarrier, suffix: "buffer" },
      ...(typedArrays ? [{ name: "Uint8Array", carrier: rustJsTypedArrayTargetType("Uint8Array"), suffix: "uint8" }] : []),
    ].flatMap(buffer => ["read", "write"].flatMap(action => positions.map(position => ({
      ...operation(`${action}Sync`, `${action}_sync_${buffer.suffix}_number`, [
        numericCarrier("Descriptor"), buffer.carrier, numericCarrier("Offset"), numericCarrier("Length"),
        rustOptionTargetType(positionCarrier),
      ], ["value", "ref", "value", "value", "value"]),
      genericParameters: [...numericGenerics("Descriptor", "Offset", "Length"), { kind: "type" as const, sourceName: positionCarrier.name,
        defaultArgument: { kind: "type" as const, type: position.carrier } }],
      signatureId: `${moduleId}::${action}Sync(${buffer.name}${position.suffix})`,
    })))),
  ];
}
