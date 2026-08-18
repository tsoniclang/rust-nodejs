import {
  boolCarrier,
  booleanType,
  fnExport,
  noneArgument,
  providerNativeFallibility,
  rustBorrowedStrToStringValueConversion,
  stringArrayType,
  stringCarrier,
  stringType,
  valueExport,
} from "../model.js";

import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";
export function pathModule(): RustProviderModuleDefinition {
  const m = "node:path";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.path",
    exports: [
      fnExport(m, "join", [{ name: "paths", type: stringArrayType, rest: true }], stringType),
      fnExport(m, "resolve", [{ name: "paths", type: stringArrayType, rest: true }], stringType),
      fnExport(m, "normalize", [{ name: "path", type: stringType }], stringType),
      fnExport(m, "dirname", [{ name: "path", type: stringType }], stringType),
      fnExport(m, "basename", [{ name: "path", type: stringType }], stringType),
      fnExport(m, "extname", [{ name: "path", type: stringType }], stringType),
      fnExport(m, "isAbsolute", [{ name: "path", type: stringType }], booleanType),
      fnExport(m, "relative", [{ name: "from", type: stringType }, { name: "to", type: stringType }], stringType),
      valueExport(m, "sep", stringType),
    ],
  };
}

export function pathRows(): readonly RustProviderOperationDefinition[] {
  const simple = (name: string, rustPath: string, resultCarrier = stringCarrier): RustProviderOperationDefinition => ({
    exportId: `node:path::${name}`,
    operationKind: "method",
    target: { form: "call", path: rustPath, argModes: ["ref"] },
    resultCarrier,
    parameterCarriers: [stringCarrier],
  });
  return [
    { exportId: "node:path::join", operationKind: "method", target: { form: "call-str-slice", path: "node_path::join" }, resultCarrier: stringCarrier },
    { exportId: "node:path::resolve", operationKind: "method", target: { form: "call-str-slice", path: "node_path::resolve" }, resultCarrier: stringCarrier, ...providerNativeFallibility },
    simple("normalize", "node_path::normalize"),
    simple("dirname", "node_path::dirname"),
    simple("extname", "node_path::extname"),
    simple("isAbsolute", "node_path::is_absolute", boolCarrier),
    {
      exportId: "node:path::relative",
      operationKind: "method",
      target: { form: "call", path: "node_path::relative", argModes: ["ref", "ref"] },
      resultCarrier: stringCarrier,
      parameterCarriers: [stringCarrier, stringCarrier],
    },
    {
      exportId: "node:path::sep",
      operationKind: "property",
      target: { form: "call", path: "node_path::sep" },
      resultCarrier: stringCarrier,
      resultConversion: rustBorrowedStrToStringValueConversion,
    },
    {
      exportId: "node:path::basename",
      operationKind: "method",
      target: { form: "call", path: "node_path::basename", argModes: ["ref"], trailingArguments: [noneArgument] },
      resultCarrier: stringCarrier,
      parameterCarriers: [stringCarrier],
    },
  ];
}

// --- node:os -----------------------------------------------------------------
