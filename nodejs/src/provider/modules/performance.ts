import { numberType } from "../model/source-types.js";
import { float64Carrier } from "../model/carriers.js";
import { providerRef, propertyMember } from "../declarations/builders.js";
import type { RustProviderModuleDefinition, RustProviderOperationDefinition, RustTargetTypeRef } from "@tsonic/target-rust/provider";

const moduleSpecifier = "node:perf_hooks";
const performanceId = `${moduleSpecifier}::Performance`;
const valueId = `${moduleSpecifier}::performance`;
export const performanceCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Performance" };

export function performanceModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier,
    providerModuleId: "node.perf_hooks",
    exports: [{
      id: performanceId, name: "Performance", kind: "interface",
      members: [propertyMember(performanceId, "timeOrigin", numberType), {
        id: `${performanceId}.now`, name: "now", kind: "method",
        signatures: [{ id: `${performanceId}.now()`, parameters: [], returnType: numberType }],
      }],
    }, {
      id: valueId, name: "performance", kind: "value",
      type: providerRef(moduleSpecifier, "Performance"),
    }],
  };
}

export function performanceRows(): readonly RustProviderOperationDefinition[] {
  return [{
    exportId: valueId, operationKind: "property",
    target: { form: "call", path: "tsonic_rust_node::perf_hooks::performance" },
    resultCarrier: performanceCarrier,
  }, {
    exportId: performanceId, memberId: `${performanceId}.timeOrigin`, operationKind: "property",
    target: { form: "field", name: "time_origin" }, receiverCarrier: performanceCarrier,
    resultCarrier: float64Carrier,
  }, {
    exportId: performanceId, memberId: `${performanceId}.now`, signatureId: `${performanceId}.now()`,
    operationKind: "method", target: { form: "receiver-method", name: "now" },
    receiverCarrier: performanceCarrier, parameterCarriers: [], resultCarrier: float64Carrier,
  }];
}
