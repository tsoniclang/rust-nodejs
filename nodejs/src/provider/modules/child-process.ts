import {
  bufferCarrier, float64Carrier, int32Carrier, nodeErrorCarrier,
  nullType, numberType, processEnvCarrier, propertyMember, providerNativeFallibility,
  providerRef, rustJsArrayTargetType, rustOptionTargetType, spawnSyncResultCarrier,
  stringArrayType, stringCarrier, stringType, unitCarrier,
} from "../model.js";
import { rustBorrowedStrToStringValueConversion, rustJsTypedArrayTargetType, rustJsStringNumberTargetType } from "@tsonic/target-rust/provider";
import type {
  ProviderTypeExpr, RustProviderModuleDefinition, RustProviderOperationDefinition, RustTargetTypeRef,
} from "../model.js";

const moduleSpecifier = "node:child_process";
const resultId = `${moduleSpecifier}::SpawnSyncReturns`;
const optionsId = `${moduleSpecifier}::SpawnSyncOptionsWithBufferEncoding`;
const errorId = `${moduleSpecifier}::SpawnSyncError`;
export const spawnOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.SpawnSyncOptions" };

function optionFields(typedArrays: boolean): readonly {
  name: string; field: string; type: ProviderTypeExpr; carrier: RustTargetTypeRef;
}[] {
  return [
    { name: "encoding", field: "encoding", type: { kind: "literal", value: "buffer" }, carrier: stringCarrier },
    { name: "cwd", field: "cwd", type: stringType, carrier: stringCarrier },
    { name: "env", field: "env", type: providerRef("node:process", "ProcessEnv"), carrier: processEnvCarrier },
    ...["maxBuffer", "uid", "gid", "timeout"].map(name => ({
      name, field: name === "maxBuffer" ? "max_buffer" : name, type: numberType, carrier: float64Carrier,
    })),
    { name: "killSignal", field: "kill_signal", type: providerRef("node:process", "Signals"), carrier: stringCarrier },
    { name: "input", field: "input", type: typedArrays ? { kind: "source-global", name: "Uint8Array" } : providerRef("node:buffer", "Buffer"), carrier: rustJsTypedArrayTargetType("Uint8Array") },
    { name: "stdio", field: "stdio", type: { kind: "array", elementType: {
      kind: "union", types: [numberType, nullType, { kind: "undefined" },
        ...["pipe", "ignore", "inherit"].map(value => ({ kind: "literal" as const, value }))],
    } }, carrier: rustJsArrayTargetType(rustJsStringNumberTargetType()) },
  ];
}

export function childProcessModule(typedArrays: boolean): RustProviderModuleDefinition {
  const nullable = (type: ProviderTypeExpr): ProviderTypeExpr => ({ kind: "union", types: [type, nullType] });
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.child-process",
    imports: [
      { moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] },
      { moduleSpecifier: "node:process", namedImports: [{ exportedName: "ProcessEnv" }, { exportedName: "Signals" }] },
    ],
    exports: [
      { id: optionsId, name: "SpawnSyncOptionsWithBufferEncoding", kind: "interface",
        members: optionFields(typedArrays).map(field => propertyMember(optionsId, field.name, field.type, { readonly: false, optional: true })) },
      { id: errorId, name: "SpawnSyncError", kind: "interface",
        members: [propertyMember(errorId, "message", stringType), propertyMember(errorId, "code", stringType)] },
      { id: resultId, name: "SpawnSyncReturns", kind: "interface", typeParameters: [{ name: "T" }],
        members: [
          ...["stdout", "stderr"].map(name => propertyMember(resultId, name, nullable({ kind: "type-parameter", name: "T" }), { readonly: false })),
          propertyMember(resultId, "status", nullable(numberType), { readonly: false }),
          propertyMember(resultId, "pid", numberType, { readonly: false, optional: true }),
          propertyMember(resultId, "signal", nullable(providerRef("node:process", "Signals")), { readonly: false }),
          propertyMember(resultId, "error", providerRef(moduleSpecifier, "SpawnSyncError"), { readonly: false, optional: true }),
        ] },
      { id: `${moduleSpecifier}::spawnSync`, name: "spawnSync", kind: "function",
        signatures: [false, true].map(options => ({
          id: `${moduleSpecifier}::spawnSync(command,args${options ? ",options" : ""})`, name: "spawnSync",
          parameters: [{ name: "command", type: stringType }, { name: "args", type: stringArrayType },
            ...(options ? [{ name: "options", type: providerRef(moduleSpecifier, "SpawnSyncOptionsWithBufferEncoding") }] : [])],
          returnType: providerRef(moduleSpecifier, "SpawnSyncReturns", [providerRef("node:buffer", "Buffer")]),
        })) },
    ],
  };
}

export function childProcessRows(typedArrays: boolean): readonly RustProviderOperationDefinition[] {
  const argumentsCarrier = { kind: "type-parameter", name: "Arguments" } as const;
  const fields = [
    ...["stdout", "stderr"].map(name => ({ owner: resultId, name, field: name, carrier: rustOptionTargetType(bufferCarrier) })),
    { owner: resultId, name: "status", field: "status", carrier: rustOptionTargetType(int32Carrier) },
    { owner: resultId, name: "pid", field: "pid", carrier: rustOptionTargetType(float64Carrier) },
    { owner: resultId, name: "signal", field: "signal", carrier: rustOptionTargetType(stringCarrier) },
    { owner: resultId, name: "error", field: "error", carrier: rustOptionTargetType(nodeErrorCarrier) },
    ...optionFields(typedArrays).map(field => ({ ...field, owner: optionsId, carrier: rustOptionTargetType(field.carrier) })),
  ];
  return [
    ...[false, true].map(options => ({
      exportId: `${moduleSpecifier}::spawnSync`,
      signatureId: `${moduleSpecifier}::spawnSync(command,args${options ? ",options" : ""})`,
      operationKind: "method" as const,
      target: { form: "call" as const, path: `node_child_process::spawn_sync_result${options ? "_with_options" : ""}`,
        argModes: ["ref" as const, "ref" as const, ...(options ? ["value" as const] : [])] },
      resultCarrier: spawnSyncResultCarrier,
      parameterCarriers: [stringCarrier, argumentsCarrier, ...(options ? [spawnOptionsCarrier] : [])],
      genericParameters: [{ kind: "type" as const, sourceName: argumentsCarrier.name }],
      ...providerNativeFallibility,
    })),
    ...fields.flatMap(field => [
      { exportId: field.owner, memberId: `${field.owner}.${field.name}`, operationKind: "property" as const,
        target: { form: "field" as const, name: field.field }, resultCarrier: field.carrier,
        receiverCarrier: field.owner === optionsId ? spawnOptionsCarrier : spawnSyncResultCarrier },
      { exportId: field.owner, memberId: `${field.owner}.${field.name}`, operationKind: "property-set" as const,
        target: { form: "field" as const, name: field.field }, resultCarrier: unitCarrier, parameterCarriers: [field.carrier],
        receiverCarrier: field.owner === optionsId ? spawnOptionsCarrier : spawnSyncResultCarrier },
    ]),
    ...["message", "code"].map(name => ({
      exportId: errorId, memberId: `${errorId}.${name}`, operationKind: "property" as const,
      target: { form: "receiver-method" as const, name }, resultCarrier: stringCarrier,
      resultConversion: rustBorrowedStrToStringValueConversion, receiverCarrier: nodeErrorCarrier,
    })),
  ];
}
