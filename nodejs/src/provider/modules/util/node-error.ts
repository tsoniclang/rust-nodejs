import { rustBorrowedStrToStringValueConversion } from "@tsonic/target-rust/provider";
import type { RustProviderModuleDefinition, RustProviderOperationDefinition } from "@tsonic/target-rust/provider";
import { propertyMember, providerRef } from "../../declarations/builders.js";
import { nodeErrorCarrier, stringCarrier } from "../../model/carriers.js";
import { stringType, undefinedType } from "../../model/source-types.js";

const exportId = "node:util::NodeError";
export const nodeErrorType = providerRef("node:util", "NodeError");
export const optionalNodeErrorType = { kind: "union", types: [nodeErrorType, undefinedType] } as const;

export function nodeErrorDeclaration(): RustProviderModuleDefinition["exports"][number] {
  return {
    id: exportId,
    name: "NodeError",
    kind: "interface",
    members: [propertyMember(exportId, "message", stringType)],
  };
}

export function nodeErrorRows(): readonly RustProviderOperationDefinition[] {
  return [{
    exportId,
    memberId: `${exportId}.message`,
    operationKind: "property",
    target: { form: "receiver-method", name: "message" },
    receiverCarrier: nodeErrorCarrier,
    resultCarrier: stringCarrier,
    resultConversion: rustBorrowedStrToStringValueConversion,
  }];
}
