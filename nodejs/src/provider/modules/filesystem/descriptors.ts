import {
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
const positionType = { kind: "union", types: [numberType, nullType] } as const;

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
      signatures: buffers.map(buffer => ({
        id: `${moduleId}::${name}(${buffer.name})`, name,
        parameters: [
          { name: "fd", type: numberType }, { name: "buffer", type: buffer.type },
          { name: "offset", type: numberType }, { name: "length", type: numberType },
          { name: "position", type: positionType },
        ],
        returnType: numberType,
      })),
    })),
  ];
}

export function fileDescriptorRows(typedArrays: boolean): readonly RustProviderOperationDefinition[] {
  const operation = (
    name: string, path: string, parameters: readonly RustTargetTypeRef[],
    modes: readonly ("ref" | "value")[], result: RustTargetTypeRef = float64Carrier,
  ): RustProviderOperationDefinition => ({
    exportId: `${moduleId}::${name}`, operationKind: "method",
    target: { form: "call", path: `node_fs::${path}`, argModes: modes },
    resultCarrier: result, parameterCarriers: parameters, ...providerNativeFallibility,
  });
  return [
    { ...operation("openSync", "open_sync_number", [stringCarrier, stringCarrier], ["ref", "ref"]), signatureId: `${moduleId}::openSync(path,flags)` },
    ...[
      { name: "string", carrier: stringCarrier, path: "open_sync_numeric" },
      { name: "Buffer", carrier: bufferCarrier, path: "open_sync_buffer_numeric" },
    ].map(path => ({
      ...operation("openSync", path.path, [path.carrier, float64Carrier, float64Carrier], ["ref", "value", "value"]),
      signatureId: `${moduleId}::openSync(${path.name},number,number)`,
    })),
    operation("closeSync", "close_sync_number", [float64Carrier], ["value"], unitCarrier),
    ...[
      { name: "Buffer", carrier: bufferCarrier, suffix: "buffer" },
      ...(typedArrays ? [{ name: "Uint8Array", carrier: rustJsTypedArrayTargetType("Uint8Array"), suffix: "uint8" }] : []),
    ].flatMap(buffer => ["read", "write"].map(action => ({
      ...operation(`${action}Sync`, `${action}_sync_${buffer.suffix}_number`, [
        float64Carrier, buffer.carrier, float64Carrier, float64Carrier,
        rustOptionTargetType(float64Carrier),
      ], ["value", "ref", "value", "value", "value"]),
      signatureId: `${moduleId}::${action}Sync(${buffer.name})`,
    }))),
  ];
}
