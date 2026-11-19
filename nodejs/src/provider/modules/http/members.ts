import type { RustProviderOperationDefinition, RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { providerCallbackType } from "../../declarations/builders.js";
import { stringCarrier } from "../../model/carriers.js";
import { providerNativeFallibility, sourceCallbackFallibility } from "../../model/operations.js";
import type { ProviderTypeExpr } from "../../model/source-types.js";

export function eventRows(
  exportId: string,
  receiverCarrier: RustTargetTypeRef,
  events: readonly (readonly [name: string, listener: RustTargetTypeRef])[],
  failure: typeof providerNativeFallibility | typeof sourceCallbackFallibility = sourceCallbackFallibility,
): readonly RustProviderOperationDefinition[] {
  return (["on", "once", "off"] as const).flatMap(methodName => events.map(([eventName, listener]) => ({
    exportId,
    memberId: `${exportId}.${methodName}`,
    signatureId: `${exportId}.${methodName}.${eventName}`,
    operationKind: "method" as const,
    target: {
      form: "receiver-method" as const,
      name: `${methodName}_${eventName}`,
      argModes: ["ref", "ref"] as const,
    },
    receiverCarrier,
    resultCarrier: receiverCarrier,
    parameterCarriers: [stringCarrier, listener],
    ...failure,
  })));
}

export function receiver(
  exportId: string,
  memberName: string,
  targetName: string,
  receiverCarrier: RustTargetTypeRef,
  resultCarrier: RustTargetTypeRef,
  parameterCarriers: readonly RustTargetTypeRef[],
  operationKind: "method" | "property" = "method",
): RustProviderOperationDefinition {
  return {
    exportId,
    memberId: `${exportId}.${memberName}`,
    operationKind,
    target: { form: "receiver-method", name: targetName },
    receiverCarrier,
    resultCarrier,
    parameterCarriers,
  };
}

export function fallibleReceiver(
  exportId: string,
  memberName: string,
  targetName: string,
  receiverCarrier: RustTargetTypeRef,
  resultCarrier: RustTargetTypeRef,
  parameterCarriers: readonly RustTargetTypeRef[],
  signatureId?: string,
  argModes?: readonly ("value" | "ref" | "mut-ref")[],
  operationKind: "method" | "property-set" = "method",
  failure: typeof providerNativeFallibility | typeof sourceCallbackFallibility = sourceCallbackFallibility,
): RustProviderOperationDefinition {
  return {
    exportId,
    memberId: `${exportId}.${memberName}`,
    ...(signatureId === undefined ? {} : { signatureId }),
    operationKind,
    target: {
      form: "receiver-method",
      name: targetName,
      ...(argModes === undefined ? {} : { argModes }),
    },
    receiverCarrier,
    resultCarrier,
    parameterCarriers,
    ...failure,
  };
}

export function field(
  exportId: string,
  memberName: string,
  receiverCarrier: RustTargetTypeRef,
  resultCarrier: RustTargetTypeRef,
): RustProviderOperationDefinition {
  return {
    exportId,
    memberId: `${exportId}.${memberName}`,
    operationKind: "property",
    target: { form: "field", name: memberName },
    receiverCarrier,
    resultCarrier,
  };
}

export function method(
  classId: string,
  name: string,
  parameters: readonly {
    readonly name: string;
    readonly type: ProviderTypeExpr;
    readonly optional?: boolean;
  }[],
  returnType: ProviderTypeExpr,
) {
  return {
    id: `${classId}.${name}`,
    name,
    kind: "method" as const,
    signatures: [{
      id: `${classId}.${name}(${parameters.map(parameter => parameter.name).join(",")})`,
      parameters,
      returnType,
    }],
  };
}

export function property(
  classId: string,
  name: string,
  type: ProviderTypeExpr,
  readonly = true,
) {
  return {
    id: `${classId}.${name}`,
    name,
    kind: "property" as const,
    ...(readonly ? { readonly: true } : {}),
    type,
  };
}

export function eventMembers(
  classId: string,
  returnType: ProviderTypeExpr,
  events: readonly (readonly [
    name: string,
    parameters: readonly {
      readonly name: string;
      readonly type: ProviderTypeExpr;
    }[],
  ])[],
) {
  return (["on", "once", "off"] as const).map(methodName => ({
    id: `${classId}.${methodName}`,
    name: methodName,
    kind: "method" as const,
    signatures: events.map(([eventName, parameters]) => ({
      id: `${classId}.${methodName}.${eventName}`,
      parameters: [
        { name: "event", type: { kind: "literal" as const, value: eventName } },
        { name: "listener", type: providerCallbackType(`${classId}.${methodName}.${eventName}`, "listener", parameters) },
      ],
      returnType,
    })),
  }));
}
