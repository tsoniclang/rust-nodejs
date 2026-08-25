import {
  fnExport,
  makeDirectoryOptionsCarrier,
  providerNativeFallibility,
  providerRef,
  rmOptionsCarrier,
  statsCarrier,
  stringArrayCarrier,
  stringArrayType,
  stringCarrier,
  stringType,
  unitCarrier,
  voidType,
} from "../model.js";

import type {
  RustProviderConstantArgument,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustTargetTypeRef,
} from "../model.js";
export function fsPromisesModule(): RustProviderModuleDefinition {
  const m = "node:fs/promises";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.fs-promises",
    imports: [{ moduleSpecifier: "node:fs", namedImports: [
      { exportedName: "Stats" },
      { exportedName: "MakeDirectoryOptions" },
      { exportedName: "RmOptions" },
    ] }],
    exports: [
      fnExport(m, "readFile", [{ name: "path", type: stringType }, { name: "encoding", type: stringType }], stringType),
      fnExport(m, "writeFile", [{ name: "path", type: stringType }, { name: "data", type: stringType }, { name: "encoding", type: stringType }], voidType),
      fnExport(m, "readdir", [{ name: "path", type: stringType }], stringArrayType),
      fnExport(m, "stat", [{ name: "path", type: stringType }], providerRef("node:fs", "Stats")),
      {
        id: `${m}::mkdir`,
        name: "mkdir",
        kind: "function" as const,
        signatures: [
          { id: `${m}::mkdir(path)`, parameters: [{ name: "path", type: stringType }], returnType: voidType },
          { id: `${m}::mkdir(path,options)`, parameters: [{ name: "path", type: stringType }, { name: "options", type: providerRef("node:fs", "MakeDirectoryOptions") }], returnType: voidType },
        ],
      },
      {
        id: `${m}::rm`,
        name: "rm",
        kind: "function" as const,
        signatures: [
          { id: `${m}::rm(path)`, parameters: [{ name: "path", type: stringType }], returnType: voidType },
          { id: `${m}::rm(path,options)`, parameters: [{ name: "path", type: stringType }, { name: "options", type: providerRef("node:fs", "RmOptions") }], returnType: voidType },
        ],
      },
      fnExport(m, "unlink", [{ name: "path", type: stringType }], voidType),
      fnExport(m, "copyFile", [{ name: "from", type: stringType }, { name: "to", type: stringType }], voidType),
      fnExport(m, "rename", [{ name: "from", type: stringType }, { name: "to", type: stringType }], voidType),
    ],
  };
}

export function fsPromisesRows(): readonly RustProviderOperationDefinition[] {
  const row = (name: string, path: string, resultCarrier: RustTargetTypeRef, parameterCount: number, trailingArguments?: readonly RustProviderConstantArgument[]): RustProviderOperationDefinition => ({
    exportId: `node:fs/promises::${name}`,
    operationKind: "method",
    target: {
      form: "call",
      path,
      argModes: Array.from({ length: parameterCount }, () => "ref" as const),
      ...(trailingArguments === undefined ? {} : { trailingArguments }),
    },
    resultCarrier,
    parameterCarriers: Array.from({ length: parameterCount }, () => stringCarrier),
    ...providerNativeFallibility,
    isAsync: true,
  });
  const unit: RustTargetTypeRef = unitCarrier;
  return [
    row("readFile", "node_fs_promises::read_file_string_async", stringCarrier, 2),
    row("writeFile", "node_fs_promises::write_file_string_async", unit, 3),
    row("readdir", "node_fs_promises::readdir_async", stringArrayCarrier, 1),
    row("stat", "node_fs_promises::stat_async", statsCarrier, 1),
    {
      ...row("mkdir", "node_fs_promises::mkdir_async", unit, 1),
      signatureId: "node:fs/promises::mkdir(path)",
    },
    {
      exportId: "node:fs/promises::mkdir",
      signatureId: "node:fs/promises::mkdir(path,options)",
      operationKind: "method",
      target: { form: "call", path: "node_fs_promises::mkdir_with_options_async", argModes: ["ref", "value"] },
      resultCarrier: unit,
      parameterCarriers: [stringCarrier, makeDirectoryOptionsCarrier],
      ...providerNativeFallibility,
      isAsync: true,
    },
    {
      ...row("rm", "node_fs_promises::rm_async", unit, 1),
      signatureId: "node:fs/promises::rm(path)",
    },
    {
      exportId: "node:fs/promises::rm",
      signatureId: "node:fs/promises::rm(path,options)",
      operationKind: "method",
      target: { form: "call", path: "node_fs_promises::rm_with_options_async", argModes: ["ref", "value"] },
      resultCarrier: unit,
      parameterCarriers: [stringCarrier, rmOptionsCarrier],
      ...providerNativeFallibility,
      isAsync: true,
    },
    row("unlink", "node_fs_promises::unlink_async", unit, 1),
    row("copyFile", "node_fs_promises::copy_file_async", unit, 2),
    row("rename", "node_fs_promises::rename_async", unit, 2),
  ];
}

// --- node:process ------------------------------------------------------------

// node:process exposes the Node module shape: cwd() as a function and
// platform/arch/argv/pid/ppid/env as value exports. env is an indexed
// object whose reads preserve absence as undefined (Option carrier).
