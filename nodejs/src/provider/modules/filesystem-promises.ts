import {
  fnExport,
  providerNativeFallibility,
  providerRef,
  statsCarrier,
  stringArrayCarrier,
  stringArrayType,
  stringCarrier,
  stringType,
  trueArgument,
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
    imports: [{ moduleSpecifier: "node:fs", namedImports: [{ exportedName: "Stats" }] }],
    exports: [
      fnExport(m, "readFile", [{ name: "path", type: stringType }, { name: "encoding", type: stringType }], stringType),
      fnExport(m, "writeFile", [{ name: "path", type: stringType }, { name: "data", type: stringType }, { name: "encoding", type: stringType }], voidType),
      fnExport(m, "readdir", [{ name: "path", type: stringType }], stringArrayType),
      fnExport(m, "stat", [{ name: "path", type: stringType }], providerRef("node:fs", "Stats")),
      // Contract: recursive.
      fnExport(m, "mkdir", [{ name: "path", type: stringType }], voidType),
      // Contract: recursive and force.
      fnExport(m, "rm", [{ name: "path", type: stringType }], voidType),
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
  const unit: RustTargetTypeRef = { kind: "tuple", elements: [] };
  return [
    row("readFile", "node_fs_promises::read_file_string_async", stringCarrier, 2),
    row("writeFile", "node_fs_promises::write_file_string_async", unit, 3),
    row("readdir", "node_fs_promises::readdir_async", stringArrayCarrier, 1),
    row("stat", "node_fs_promises::stat_async", statsCarrier, 1),
    row("mkdir", "node_fs_promises::mkdir_async", unit, 1, [trueArgument]),
    row("rm", "node_fs_promises::rm_async", unit, 1, [trueArgument, trueArgument]),
    row("unlink", "node_fs_promises::unlink_async", unit, 1),
    row("copyFile", "node_fs_promises::copy_file_async", unit, 2),
    row("rename", "node_fs_promises::rename_async", unit, 2),
  ];
}

// --- node:process ------------------------------------------------------------

// node:process exposes the Node module shape: cwd() as a function and
// platform/arch/argv/pid/ppid/env as value exports. env is an indexed
// object whose reads preserve absence as undefined (Option carrier).
