import { fnExport, propertyMember, providerRef } from "../declarations/builders.js";
import { float64Carrier, stringCarrier, unitCarrier } from "../model/carriers.js";
import { numberType, stringType, voidType } from "../model/source-types.js";
import { providerNativeFallibility } from "../model/operations.js";
import type { RustProviderModuleDefinition, RustProviderOperationDefinition, RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const v8HeapInfoCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HeapInfo" };

const heapFields = [
  "total_heap_size", "total_heap_size_executable", "total_physical_size",
  "total_available_size", "used_heap_size", "heap_size_limit", "malloced_memory",
  "peak_malloced_memory", "number_of_native_contexts", "number_of_detached_contexts",
  "total_global_handles_size", "used_global_handles_size", "external_memory", "total_allocated_bytes",
];
const heapMembers = [
  ...heapFields.map(name => propertyMember("node:v8::HeapInfo", name, numberType, { readonly: false })),
  propertyMember("node:v8::HeapInfo", "does_zap_garbage", {
    kind: "union", types: [{ kind: "literal", value: 0 }, { kind: "literal", value: 1 }],
  }, { readonly: false }),
];

export function v8Module(): RustProviderModuleDefinition {
  return {
    moduleSpecifier: "node:v8",
    providerModuleId: "tsonic.rust.node.v8",
    exports: [
      fnExport("node:v8", "setFlagsFromString", [{ name: "flags", type: stringType }], voidType),
      fnExport("node:v8", "getHeapStatistics", [], providerRef("node:v8", "HeapInfo")),
      { id: "node:v8::HeapInfo", name: "HeapInfo", kind: "interface", members: heapMembers },
    ],
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
  }, {
    exportId: "node:v8::getHeapStatistics",
    operationKind: "method",
    target: { form: "call", path: "tsonic_rust_node::v8::get_heap_statistics" },
    parameterCarriers: [],
    resultCarrier: v8HeapInfoCarrier,
    ...providerNativeFallibility,
  }, ...heapMembers.flatMap(member => [{
    exportId: "node:v8::HeapInfo", memberId: member.id, operationKind: "property" as const,
    target: { form: "field" as const, name: member.name },
    receiverCarrier: v8HeapInfoCarrier, resultCarrier: float64Carrier,
  }, {
    exportId: "node:v8::HeapInfo", memberId: member.id, operationKind: "property-set" as const,
    target: { form: "field" as const, name: member.name },
    receiverCarrier: v8HeapInfoCarrier, parameterCarriers: [float64Carrier], resultCarrier: unitCarrier,
  }])];
}
