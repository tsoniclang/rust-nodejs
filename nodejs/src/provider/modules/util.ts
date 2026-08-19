import {
  fnExport,
  int32Carrier,
  jsValueCarrier,
  numberType,
  rustBorrowedStrToStringValueConversion,
  stringCarrier,
  stringType,
} from "../model.js";

import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";
export function utilModule(): RustProviderModuleDefinition {
  const m = "node:util";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.util",
    exports: [
      fnExport(m, "stripVTControlCharacters", [{ name: "value", type: stringType }], stringType),
      fnExport(m, "toUSVString", [{ name: "value", type: stringType }], stringType),
      fnExport(m, "styleText", [{ name: "style", type: stringType }, { name: "text", type: stringType }], stringType),
      fnExport(m, "getSystemErrorName", [{ name: "code", type: numberType }], stringType),
      fnExport(m, "getSystemErrorMessage", [{ name: "code", type: numberType }], stringType),
      fnExport(m, "inspect", [{ name: "value", type: { kind: "any" } }], stringType),
      fnExport(m, "format", [{ name: "format", type: stringType }, { name: "values", type: { kind: "array", elementType: { kind: "any" } }, rest: true }], stringType),
    ],
  };
}

export function utilRows(): readonly RustProviderOperationDefinition[] {
  const m = "node:util";
  return [
    { exportId: `${m}::stripVTControlCharacters`, operationKind: "method", target: { form: "call", path: "node_util::strip_vt_control_characters", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier] },
    { exportId: `${m}::toUSVString`, operationKind: "method", target: { form: "call", path: "node_util::to_usv_string", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier] },
    { exportId: `${m}::styleText`, operationKind: "method", target: { form: "call", path: "node_util::style_text", argModes: ["ref", "ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier, stringCarrier] },
    { exportId: `${m}::getSystemErrorName`, operationKind: "method", target: { form: "call", path: "node_util::get_system_error_name" }, resultCarrier: stringCarrier, resultConversion: rustBorrowedStrToStringValueConversion, parameterCarriers: [int32Carrier] },
    { exportId: `${m}::inspect`, operationKind: "method", target: { form: "call", path: "node_util::inspect", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [jsValueCarrier] },
    { exportId: `${m}::getSystemErrorMessage`, operationKind: "method", target: { form: "call", path: "node_util::get_system_error_message" }, resultCarrier: stringCarrier, resultConversion: rustBorrowedStrToStringValueConversion, parameterCarriers: [int32Carrier] },
    { exportId: `${m}::format`, operationKind: "method", target: { form: "call-value-slice", path: "node_util::format", leadingArguments: [{ carrier: stringCarrier, mode: "ref" }], elementCarrier: jsValueCarrier }, resultCarrier: stringCarrier },
  ];
}
