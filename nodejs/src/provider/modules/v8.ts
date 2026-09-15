import {
  fnExport,
  providerNativeFallibility,
  stringCarrier,
  stringType,
  unitCarrier,
  voidType,
} from "../model.js";
import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";

export function v8Module(): RustProviderModuleDefinition {
  return {
    moduleSpecifier: "node:v8",
    providerModuleId: "tsonic.rust.node.v8",
    exports: [fnExport("node:v8", "setFlagsFromString", [{ name: "flags", type: stringType }], voidType)],
  };
}

export function v8Rows(): readonly RustProviderOperationDefinition[] {
  return [{
    exportId: "node:v8::setFlagsFromString",
    operationKind: "method",
    target: { form: "call", path: "tsonic_rust_node::v8::set_flags_from_string", argModes: ["ref"] },
    parameterCarriers: [stringCarrier],
    resultCarrier: unitCarrier,
    ...providerNativeFallibility,
  }];
}
