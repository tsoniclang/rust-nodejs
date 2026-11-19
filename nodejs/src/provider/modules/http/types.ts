import {
  rustCallableTargetType,
  rustJsArrayTargetType,
  rustOptionTargetType,
  rustRetainedErrorTargetType,
} from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { providerRef } from "../../declarations/builders.js";
import {
  int32Carrier,
  nodeErrorCarrier,
  stringCarrier,
  unitCarrier,
} from "../../model/carriers.js";
import {
  int32Type,
  stringType,
  undefinedType,
  voidType,
} from "../../model/source-types.js";
import type { ProviderTypeExpr } from "../../model/source-types.js";
import { bufferCarrier } from "../buffer/carriers.js";
import { netSocketCarrier } from "../net/carriers.js";
import { nodeErrorType } from "../util/node-error.js";
import { httpAddressInfoCarrier } from "./carriers.js";

export const moduleSpecifier = "node:http";
export const incomingHeadersId = `${moduleSpecifier}::IncomingHttpHeaders`;
export const incomingHeaderValuesId = `${moduleSpecifier}::IncomingHttpHeaderValues`;
export const outgoingHeadersId = `${moduleSpecifier}::OutgoingHttpHeaders`;
export const addressInfoId = `${moduleSpecifier}::AddressInfo`;
export const serverAddressId = `${moduleSpecifier}::ServerAddress`;
export const incomingId = `${moduleSpecifier}::IncomingMessage`;
export const responseId = `${moduleSpecifier}::ServerResponse`;
export const serverId = `${moduleSpecifier}::Server`;
export const bufferType = providerRef("node:buffer", "Buffer");
export const errorType = nodeErrorType;
export const optionalStringType = optional(stringType);
export const optionalInt32Type = optional(int32Type);
export const stringArrayType = { kind: "array", elementType: stringType } as const;
export const stringArrayCarrier = rustJsArrayTargetType(stringCarrier);
export const borrowedHeaderValuesType = {
  kind: "source-global", name: "ReadonlyArray", typeArguments: [stringType],
} as const;
export const borrowedHeaderValuesCarrier: RustTargetTypeRef = {
  kind: "reference", mutable: false, referent: { kind: "slice", element: stringCarrier },
};
export const emptyListenerCarrier = rustCallableTargetType([], unitCarrier);
export const errorListenerCarrier = rustCallableTargetType([nodeErrorCarrier], unitCarrier);
export const retainedErrorCarrier = rustRetainedErrorTargetType();
export const retainedErrorListenerCarrier = rustCallableTargetType([retainedErrorCarrier], unitCarrier);
export const dataListenerCarrier = rustCallableTargetType([bufferCarrier], unitCarrier);
export const optionalErrorCarrier = rustOptionTargetType(retainedErrorCarrier);
export const optionalStringCarrier = rustOptionTargetType(stringCarrier);
export const optionalInt32Carrier = rustOptionTargetType(int32Carrier);
export const optionalAddressCarrier = rustOptionTargetType(httpAddressInfoCarrier);
export const socketReferenceCarrier: RustTargetTypeRef = {
  kind: "reference",
  referent: netSocketCarrier,
  mutable: false,
};

export function callbackType(
  signatureId: string,
  parameters: readonly {
    readonly name: string;
    readonly type: ProviderTypeExpr;
    readonly optional?: boolean;
  }[],
): ProviderTypeExpr {
  return {
    kind: "function",
    id: `${signatureId}::callback`,
    parameters,
    returnType: voidType,
  };
}

export function optional(type: ProviderTypeExpr): ProviderTypeExpr {
  return { kind: "union", types: [type, undefinedType] };
}
