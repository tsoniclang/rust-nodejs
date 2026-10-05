import { rustOptionTargetType } from "@tsonic/target-rust/provider";
import type { RustProviderModuleDefinition, RustProviderOperationDefinition, RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { stringCarrier } from "../../model/carriers.js";
import { providerNativeFallibility } from "../../model/operations.js";
import { stringType } from "../../model/source-types.js";
import { incomingHttpHeadersCarrier } from "./carriers.js";
import {
  incomingHeaderValuesId,
  optionalStringType,
  stringArrayType,
  stringArrayCarrier,
  borrowedHeaderValuesType,
  borrowedHeaderValuesCarrier,
  optionalStringCarrier,
  optional,
} from "./types.js";
import { receiver, fallibleReceiver, method } from "./members.js";

export function headerDeclaration(id: string, name: string): RustProviderModuleDefinition["exports"][number] {
  return {
    id,
    name,
    kind: "class",
    members: [
      method(id, "get", [{ name: "name", type: stringType }], optionalStringType),
      method(id, "getAll", [{ name: "name", type: stringType }], stringArrayType),
      method(id, "names", [], stringArrayType),
    ],
  };
}

export function headerValuesDeclaration(): RustProviderModuleDefinition["exports"][number] {
  return {
    id: incomingHeaderValuesId,
    name: "IncomingHttpHeaderValues",
    kind: "class",
    members: [{
      id: `${incomingHeaderValuesId}.indexer`,
      name: "indexer",
      kind: "indexer",
      signatures: [{
        id: `${incomingHeaderValuesId}.indexer(name)`,
        parameters: [{ name: "name", type: stringType }],
        returnType: optional(borrowedHeaderValuesType),
      }],
    }],
  };
}

export function headerRows(
  exportId: string,
  receiverCarrier: RustTargetTypeRef,
): readonly RustProviderOperationDefinition[] {
  return [
    fallibleReceiver(exportId, "get", "get", receiverCarrier, optionalStringCarrier, [stringCarrier], undefined, ["ref"]),
    fallibleReceiver(exportId, "getAll", "get_all", receiverCarrier, stringArrayCarrier, [stringCarrier], undefined, ["ref"]),
    receiver(exportId, "names", "names", receiverCarrier, stringArrayCarrier, []),
  ];
}

export function headerValuesRow(): RustProviderOperationDefinition {
  return {
    exportId: incomingHeaderValuesId,
    memberId: `${incomingHeaderValuesId}.indexer`,
    operationKind: "indexer",
    target: { form: "receiver-method", name: "get_values", argModes: ["ref"] },
    receiverCarrier: incomingHttpHeadersCarrier,
    resultCarrier: rustOptionTargetType(borrowedHeaderValuesCarrier),
    parameterCarriers: [stringCarrier],
    ...providerNativeFallibility,
  };
}
