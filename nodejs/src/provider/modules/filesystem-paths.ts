import {
  boolCarrier, booleanType, bufferCarrier, float64Carrier,
  methodMember, numberType, propertyMember, providerNativeFallibility,
  providerRef, rustOptionTargetType, statsCarrier, stringArrayCarrier,
  stringArrayType, stringCarrier, stringType, unitCarrier, valueExport, voidType,
} from "../model.js";
import { rustInt32ToFloat64ValueConversion, rustJsArrayTargetType } from "@tsonic/target-rust/provider";
import type {
  RustProviderModuleDefinition, RustProviderOperationDefinition, RustTargetTypeRef,
} from "../model.js";

const moduleId = "node:fs";
const statOptionsId = `${moduleId}::StatOptions`;
const directoryOptionsId = `${moduleId}::BufferDirectoryOptions`;
const direntId = `${moduleId}::Dirent`;
const constantsId = `${moduleId}::FsConstants`;
const nameParameter = { kind: "type-parameter", name: "Name" } as const;
export const direntGenerics = [{ kind: "type", sourceName: "Name", defaultArgument: { kind: "type", type: stringCarrier } }] as const;
export const statOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.StatOptions" };
export const directoryOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.BufferDirectoryOptions" };
export const fsConstantsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.FsConstants" };
export function direntCarrier(name: RustTargetTypeRef): RustTargetTypeRef {
  return { kind: "target-named", id: "rust.node.Dirent", genericArguments: [{ kind: "type", type: name }] };
}
const paths = [
  { name: "path", type: stringType, carrier: stringCarrier, suffix: "" },
  { name: "bufferPath", type: providerRef("node:buffer", "Buffer"), carrier: bufferCarrier, suffix: "_buffer" },
] as const;
const constants = [
  { name: "O_APPEND", field: "o_append" },
  { name: "O_CREAT", field: "o_creat" },
  { name: "O_TRUNC", field: "o_trunc" },
  { name: "O_WRONLY", field: "o_wronly" },
] as const;
const kinds = [
  { name: "isFile", method: "is_file" }, { name: "isDirectory", method: "is_directory" },
  { name: "isSymbolicLink", method: "is_symbolic_link" }, { name: "isBlockDevice", method: "is_block_device" },
  { name: "isCharacterDevice", method: "is_character_device" }, { name: "isFIFO", method: "is_fifo" },
  { name: "isSocket", method: "is_socket" },
] as const;

export function filePathExports(): RustProviderModuleDefinition["exports"] {
  const statsType = providerRef(moduleId, "Stats");
  return [
    {
      id: statOptionsId, name: "StatOptions", kind: "interface",
      members: [
        propertyMember(statOptionsId, "bigint", { kind: "literal", value: false }, { optional: true, readonly: false }),
        propertyMember(statOptionsId, "throwIfNoEntry", booleanType, { optional: true, readonly: false }),
      ],
    },
    ...["statSync", "lstatSync"].map(name => ({
      id: `${moduleId}::${name}`, name, kind: "function" as const,
      signatures: paths.flatMap(path => [
        { id: `${moduleId}::${name}(${path.name})`, name, parameters: [{ name: "path", type: path.type }], returnType: statsType },
        { id: `${moduleId}::${name}(${path.name},options)`, name, parameters: [
          { name: "path", type: path.type }, { name: "options", type: providerRef(moduleId, "StatOptions") },
        ], returnType: { kind: "union" as const, types: [statsType, { kind: "undefined" as const }] } },
      ]),
    })),
    {
      id: directoryOptionsId, name: "BufferDirectoryOptions", kind: "interface",
      members: [
        propertyMember(directoryOptionsId, "withFileTypes", { kind: "literal", value: true }, { readonly: false }),
        propertyMember(directoryOptionsId, "encoding", { kind: "literal", value: "buffer" }, { readonly: false }),
      ],
    },
    {
      id: direntId, name: "Dirent", kind: "class",
      typeParameters: [{ name: "Name", defaultType: stringType }],
      members: [propertyMember(direntId, "name", nameParameter), ...kinds.map(kind => methodMember(direntId, kind.name, [], booleanType))],
    },
    {
      id: `${moduleId}::readdirSync`, name: "readdirSync", kind: "function",
      signatures: paths.flatMap(path => [
        { id: `${moduleId}::readdirSync(${path.name})`, name: "readdirSync", parameters: [{ name: "path", type: path.type }], returnType: stringArrayType },
        { id: `${moduleId}::readdirSync(${path.name},bufferEntries)`, name: "readdirSync", parameters: [
          { name: "path", type: path.type }, { name: "options", type: providerRef(moduleId, "BufferDirectoryOptions") },
        ], returnType: { kind: "array" as const, elementType: providerRef(moduleId, "Dirent", [providerRef("node:buffer", "Buffer")]) } },
      ]),
    },
    ...["rmdirSync", "utimesSync"].map(name => ({
      id: `${moduleId}::${name}`, name, kind: "function" as const,
      signatures: paths.map(path => ({
        id: `${moduleId}::${name}(${path.name})`, name, parameters: [
          { name: "path", type: path.type },
          ...(name === "utimesSync" ? [{ name: "atime", type: numberType }, { name: "mtime", type: numberType }] : []),
        ], returnType: voidType,
      })),
    })),
    { id: constantsId, name: "FsConstants", kind: "interface", members: constants.map(value => propertyMember(constantsId, value.name, numberType)) },
    valueExport(moduleId, "constants", providerRef(moduleId, "FsConstants")),
  ];
}

export function filePathRows(): readonly RustProviderOperationDefinition[] {
  const call = (
    name: string, signature: string, nativePath: string, result: RustTargetTypeRef,
    parameters: readonly RustTargetTypeRef[],
  ): RustProviderOperationDefinition => ({
    exportId: `${moduleId}::${name}`, signatureId: `${moduleId}::${name}(${signature})`, operationKind: "method",
    target: { form: "call", path: `node_fs::${nativePath}`, argModes: parameters.map((_, index) => index === 0 ? "ref" : "value") },
    resultCarrier: result, parameterCarriers: parameters, ...providerNativeFallibility,
  });
  const property = (owner: string, name: string, field: string, receiver: RustTargetTypeRef, result: RustTargetTypeRef): RustProviderOperationDefinition => ({
    exportId: owner, memberId: `${owner}.${name}`, operationKind: "property",
    target: { form: "field", name: field }, receiverCarrier: receiver, resultCarrier: result,
  });
  const optionBool = rustOptionTargetType(boolCarrier);
  return [
    ...paths.flatMap(path => [
      ...[{ name: "statSync", native: "stat_sync" }, { name: "lstatSync", native: "lstat_sync" }].flatMap(operation => [
        call(operation.name, path.name, `${operation.native}${path.suffix}`, statsCarrier, [path.carrier]),
        call(operation.name, `${path.name},options`, `${operation.native}${path.suffix}_with_options`, rustOptionTargetType(statsCarrier), [path.carrier, statOptionsCarrier]),
      ]),
      call("rmdirSync", path.name, `rmdir_sync${path.suffix}`, unitCarrier, [path.carrier]),
      call("utimesSync", path.name, `utimes_sync${path.suffix}`, unitCarrier, [path.carrier, float64Carrier, float64Carrier]),
      call("readdirSync", path.name, path.suffix === "" ? "readdir_sync" : "readdir_sync_buffer_path", stringArrayCarrier, [path.carrier]),
      call("readdirSync", `${path.name},bufferEntries`, path.suffix === "" ? "readdir_sync_buffer_entries" : "readdir_sync_buffer_path_entries", rustJsArrayTargetType(direntCarrier(bufferCarrier)), [path.carrier, directoryOptionsCarrier]),
    ]),
    ...[
      { owner: statOptionsId, receiver: statOptionsCarrier, name: "bigint", field: "bigint", carrier: optionBool },
      { owner: statOptionsId, receiver: statOptionsCarrier, name: "throwIfNoEntry", field: "throw_if_no_entry", carrier: optionBool },
      { owner: directoryOptionsId, receiver: directoryOptionsCarrier, name: "withFileTypes", field: "with_file_types", carrier: boolCarrier },
      { owner: directoryOptionsId, receiver: directoryOptionsCarrier, name: "encoding", field: "encoding", carrier: stringCarrier },
    ].flatMap(field => [
      property(field.owner, field.name, field.field, field.receiver, field.carrier),
      { exportId: field.owner, memberId: `${field.owner}.${field.name}`, operationKind: "property-set" as const, target: { form: "field" as const, name: field.field }, receiverCarrier: field.receiver, resultCarrier: unitCarrier, parameterCarriers: [field.carrier] },
    ]),
    { ...property(direntId, "name", "name", direntCarrier(nameParameter), nameParameter), genericParameters: direntGenerics },
    ...kinds.map(kind => ({
      exportId: direntId, memberId: `${direntId}.${kind.name}`, signatureId: `${direntId}.${kind.name}()`, operationKind: "method" as const,
      target: { form: "method" as const, name: kind.method },
      resultCarrier: boolCarrier, receiverCarrier: direntCarrier(nameParameter), genericParameters: direntGenerics,
    })),
    { exportId: `${moduleId}::constants`, operationKind: "property", target: { form: "call", path: "node_fs::constants" }, resultCarrier: fsConstantsCarrier },
    ...constants.map(value => ({ ...property(constantsId, value.name, value.field, fsConstantsCarrier, float64Carrier), resultConversion: rustInt32ToFloat64ValueConversion })),
    { exportId: `${moduleId}::Stats`, memberId: `${moduleId}::Stats.mode`, operationKind: "property", target: { form: "method", name: "mode_number" }, receiverCarrier: statsCarrier, resultCarrier: float64Carrier },
  ];
}
