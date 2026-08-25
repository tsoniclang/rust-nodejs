import {
  boolCarrier,
  booleanType,
  bufferCarrier,
  float64Carrier,
  fnExport,
  makeDirectoryOptionsCarrier,
  methodMember,
  numberType,
  propertyMember,
  providerNativeFallibility,
  providerRef,
  rmOptionsCarrier,
  rustOptionTargetType,
  rustUint64ToFloat64ValueConversion,
  statsCarrier,
  stringArrayCarrier,
  stringArrayType,
  stringCarrier,
  stringType,
  unitCarrier,
  unsupportedFn,
  voidType,
} from "../model.js";

import type {
  RustProviderConstantArgument,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustTargetTypeRef,
} from "../model.js";
export function fsModule(): RustProviderModuleDefinition {
  const m = "node:fs";
  const statsId = "node:fs::Stats";
  const makeDirectoryOptionsId = `${m}::MakeDirectoryOptions`;
  const rmOptionsId = `${m}::RmOptions`;
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.fs",
    imports: [{ moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] }],
    exports: [
      fnExport(m, "existsSync", [{ name: "path", type: stringType }], booleanType),
      {
        id: `${m}::readFileSync`,
        name: "readFileSync",
        kind: "function" as const,
        signatures: [
          {
            id: `${m}::readFileSync(path)`,
            name: "readFileSync",
            parameters: [{ name: "path", type: stringType }],
            returnType: providerRef("node:buffer", "Buffer"),
          },
          {
            id: `${m}::readFileSync(path,encoding)`,
            name: "readFileSync",
            parameters: [{ name: "path", type: stringType }, { name: "encoding", type: stringType }],
            returnType: stringType,
          },
        ],
      },
      {
        id: `${m}::writeFileSync`,
        name: "writeFileSync",
        kind: "function" as const,
        signatures: [
          {
            id: `${m}::writeFileSync(path,data,encoding)`,
            name: "writeFileSync",
            parameters: [
              { name: "path", type: stringType },
              { name: "data", type: stringType },
              { name: "encoding", type: stringType },
            ],
            returnType: voidType,
          },
          {
            id: `${m}::writeFileSync(path,buffer)`,
            name: "writeFileSync",
            parameters: [
              { name: "path", type: stringType },
              { name: "data", type: providerRef("node:buffer", "Buffer") },
            ],
            returnType: voidType,
          },
        ],
      },
      fnExport(m, "readdirSync", [{ name: "path", type: stringType }], stringArrayType),
      fnExport(m, "statSync", [{ name: "path", type: stringType }], providerRef(m, "Stats")),
      fnExport(m, "lstatSync", [{ name: "path", type: stringType }], providerRef(m, "Stats")),
      {
        id: makeDirectoryOptionsId,
        name: "MakeDirectoryOptions",
        kind: "interface" as const,
        members: [
          propertyMember(makeDirectoryOptionsId, "recursive", booleanType, {
            readonly: false,
            optional: true,
          }),
          propertyMember(makeDirectoryOptionsId, "mode", numberType, {
            readonly: false,
            optional: true,
          }),
        ],
      },
      {
        id: rmOptionsId,
        name: "RmOptions",
        kind: "interface" as const,
        members: [
          propertyMember(rmOptionsId, "recursive", booleanType, {
            readonly: false,
            optional: true,
          }),
          propertyMember(rmOptionsId, "force", booleanType, {
            readonly: false,
            optional: true,
          }),
          propertyMember(rmOptionsId, "maxRetries", numberType, {
            readonly: false,
            optional: true,
          }),
          propertyMember(rmOptionsId, "retryDelay", numberType, {
            readonly: false,
            optional: true,
          }),
        ],
      },
      {
        id: `${m}::mkdirSync`,
        name: "mkdirSync",
        kind: "function" as const,
        signatures: [
          { id: `${m}::mkdirSync(path)`, name: "mkdirSync", parameters: [{ name: "path", type: stringType }], returnType: voidType },
          { id: `${m}::mkdirSync(path,options)`, name: "mkdirSync", parameters: [{ name: "path", type: stringType }, { name: "options", type: providerRef(m, "MakeDirectoryOptions") }], returnType: voidType },
        ],
      },
      {
        id: `${m}::rmSync`,
        name: "rmSync",
        kind: "function" as const,
        signatures: [
          { id: `${m}::rmSync(path)`, name: "rmSync", parameters: [{ name: "path", type: stringType }], returnType: voidType },
          { id: `${m}::rmSync(path,options)`, name: "rmSync", parameters: [{ name: "path", type: stringType }, { name: "options", type: providerRef(m, "RmOptions") }], returnType: voidType },
        ],
      },
      fnExport(m, "mkdtempSync", [{ name: "prefix", type: stringType }], stringType),
      fnExport(m, "unlinkSync", [{ name: "path", type: stringType }], voidType),
      fnExport(m, "symlinkSync", [{ name: "target", type: stringType }, { name: "path", type: stringType }], voidType),
      fnExport(m, "copyFileSync", [{ name: "from", type: stringType }, { name: "to", type: stringType }], voidType),
      fnExport(m, "renameSync", [{ name: "from", type: stringType }, { name: "to", type: stringType }], voidType),
      fnExport(m, "realpathSync", [{ name: "path", type: stringType }], stringType),
      {
        id: statsId,
        name: "Stats",
        kind: "class" as const,
        members: [
          methodMember(statsId, "isFile", [], booleanType),
          methodMember(statsId, "isDirectory", [], booleanType),
          methodMember(statsId, "isSymbolicLink", [], booleanType),
          propertyMember(statsId, "size", numberType),
          propertyMember(statsId, "mtimeMs", numberType),
        ],
      },
      unsupportedFn(m, "watch", "a filesystem event subscription contract"),
      unsupportedFn(m, "createReadStream", "a stream carrier contract"),
      unsupportedFn(m, "createWriteStream", "a stream carrier contract"),
    ],
  };
}

export function fsRows(): readonly RustProviderOperationDefinition[] {
  const statsId = "node:fs::Stats";
  const makeDirectoryOptionsId = "node:fs::MakeDirectoryOptions";
  const rmOptionsId = "node:fs::RmOptions";
  const optionBool = rustOptionTargetType(boolCarrier);
  const optionNumber = rustOptionTargetType(float64Carrier);
  const fallible = (name: string, path: string, resultCarrier: RustTargetTypeRef, parameterCarriers: readonly RustTargetTypeRef[], trailingArguments?: readonly RustProviderConstantArgument[]): RustProviderOperationDefinition => ({
    exportId: `node:fs::${name}`,
    operationKind: "method",
    target: { form: "call", path, argModes: parameterCarriers.map(() => "ref"), ...(trailingArguments === undefined ? {} : { trailingArguments }) },
    resultCarrier,
    parameterCarriers,
    ...providerNativeFallibility,
  });
  return [
    { exportId: "node:fs::existsSync", operationKind: "method", target: { form: "call", path: "node_fs::exists_sync", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [stringCarrier] },
    {
      ...fallible("readFileSync", "node_fs::read_file_sync_buffer", bufferCarrier, [stringCarrier]),
      signatureId: "node:fs::readFileSync(path)",
    },
    {
      ...fallible("readFileSync", "node_fs::read_file_sync_string", stringCarrier, [stringCarrier, stringCarrier]),
      signatureId: "node:fs::readFileSync(path,encoding)",
    },
    {
      ...fallible("writeFileSync", "node_fs::write_file_sync_string", unitCarrier, [stringCarrier, stringCarrier, stringCarrier]),
      signatureId: "node:fs::writeFileSync(path,data,encoding)",
    },
    {
      ...fallible("writeFileSync", "node_fs::write_file_sync_buffer", unitCarrier, [stringCarrier, bufferCarrier]),
      signatureId: "node:fs::writeFileSync(path,buffer)",
    },
    fallible("readdirSync", "node_fs::readdir_sync", stringArrayCarrier, [stringCarrier]),
    fallible("statSync", "node_fs::stat_sync", statsCarrier, [stringCarrier]),
    fallible("lstatSync", "node_fs::lstat_sync", statsCarrier, [stringCarrier]),
    {
      ...fallible("mkdirSync", "node_fs::mkdir_sync", unitCarrier, [stringCarrier]),
      signatureId: "node:fs::mkdirSync(path)",
    },
    {
      exportId: "node:fs::mkdirSync",
      signatureId: "node:fs::mkdirSync(path,options)",
      operationKind: "method",
      target: { form: "call", path: "node_fs::mkdir_sync_with_options", argModes: ["ref", "value"] },
      resultCarrier: unitCarrier,
      parameterCarriers: [stringCarrier, makeDirectoryOptionsCarrier],
      ...providerNativeFallibility,
    },
    {
      ...fallible("rmSync", "node_fs::rm_sync", unitCarrier, [stringCarrier]),
      signatureId: "node:fs::rmSync(path)",
    },
    {
      exportId: "node:fs::rmSync",
      signatureId: "node:fs::rmSync(path,options)",
      operationKind: "method",
      target: { form: "call", path: "node_fs::rm_sync_with_options", argModes: ["ref", "value"] },
      resultCarrier: unitCarrier,
      parameterCarriers: [stringCarrier, rmOptionsCarrier],
      ...providerNativeFallibility,
    },
    { exportId: makeDirectoryOptionsId, memberId: `${makeDirectoryOptionsId}.recursive`, operationKind: "property", target: { form: "field", name: "recursive" }, resultCarrier: optionBool, receiverCarrier: makeDirectoryOptionsCarrier },
    { exportId: makeDirectoryOptionsId, memberId: `${makeDirectoryOptionsId}.recursive`, operationKind: "property-set", target: { form: "field", name: "recursive" }, resultCarrier: unitCarrier, parameterCarriers: [optionBool], receiverCarrier: makeDirectoryOptionsCarrier },
    { exportId: makeDirectoryOptionsId, memberId: `${makeDirectoryOptionsId}.mode`, operationKind: "property", target: { form: "field", name: "mode" }, resultCarrier: optionNumber, receiverCarrier: makeDirectoryOptionsCarrier },
    { exportId: makeDirectoryOptionsId, memberId: `${makeDirectoryOptionsId}.mode`, operationKind: "property-set", target: { form: "field", name: "mode" }, resultCarrier: unitCarrier, parameterCarriers: [optionNumber], receiverCarrier: makeDirectoryOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.recursive`, operationKind: "property", target: { form: "field", name: "recursive" }, resultCarrier: optionBool, receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.recursive`, operationKind: "property-set", target: { form: "field", name: "recursive" }, resultCarrier: unitCarrier, parameterCarriers: [optionBool], receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.force`, operationKind: "property", target: { form: "field", name: "force" }, resultCarrier: optionBool, receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.force`, operationKind: "property-set", target: { form: "field", name: "force" }, resultCarrier: unitCarrier, parameterCarriers: [optionBool], receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.maxRetries`, operationKind: "property", target: { form: "field", name: "max_retries" }, resultCarrier: optionNumber, receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.maxRetries`, operationKind: "property-set", target: { form: "field", name: "max_retries" }, resultCarrier: unitCarrier, parameterCarriers: [optionNumber], receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.retryDelay`, operationKind: "property", target: { form: "field", name: "retry_delay_ms" }, resultCarrier: optionNumber, receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.retryDelay`, operationKind: "property-set", target: { form: "field", name: "retry_delay_ms" }, resultCarrier: unitCarrier, parameterCarriers: [optionNumber], receiverCarrier: rmOptionsCarrier },
    fallible("mkdtempSync", "node_fs::mkdtemp_sync", stringCarrier, [stringCarrier]),
    fallible("unlinkSync", "node_fs::unlink_sync", unitCarrier, [stringCarrier]),
    fallible("symlinkSync", "node_fs::symlink_sync", unitCarrier, [stringCarrier, stringCarrier]),
    fallible("copyFileSync", "node_fs::copy_file_sync", unitCarrier, [stringCarrier, stringCarrier]),
    fallible("renameSync", "node_fs::rename_sync", unitCarrier, [stringCarrier, stringCarrier]),
    fallible("realpathSync", "node_fs::realpath_sync", stringCarrier, [stringCarrier]),
    { exportId: statsId, memberId: `${statsId}.isFile`, operationKind: "method", target: { form: "receiver-method", name: "is_file" }, resultCarrier: boolCarrier },
    { exportId: statsId, memberId: `${statsId}.isDirectory`, operationKind: "method", target: { form: "receiver-method", name: "is_directory" }, resultCarrier: boolCarrier },
    { exportId: statsId, memberId: `${statsId}.isSymbolicLink`, operationKind: "method", target: { form: "receiver-method", name: "is_symbolic_link" }, resultCarrier: boolCarrier },
    { exportId: statsId, memberId: `${statsId}.size`, operationKind: "property", target: { form: "field", name: "size" }, resultCarrier: float64Carrier, resultConversion: rustUint64ToFloat64ValueConversion },
    { exportId: statsId, memberId: `${statsId}.mtimeMs`, operationKind: "property", target: { form: "receiver-method", name: "mtime_ms" }, resultCarrier: float64Carrier },
  ];
}

// --- node:fs/promises --------------------------------------------------------
