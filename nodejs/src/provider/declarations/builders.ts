import { voidType } from "../model/source-types.js";
import type { ProviderTypeExpr } from "../model/source-types.js";

// Node is a provider package, not a compiler surface. Supported rows map to
// closed tsonic_rust_node APIs with exact declaration identities; every
// declared export without a row fails closed with a deterministic
// diagnostic that names the selected identity. Unsupported rows each state
// the concrete contract they require.

export function providerRef(
  moduleSpecifier: string,
  exportName: string,
  typeArguments?: readonly ProviderTypeExpr[],
): ProviderTypeExpr {
  return {
    kind: "provider-ref",
    moduleSpecifier,
    exportName,
    ...(typeArguments === undefined ? {} : { typeArguments }),
  };
}

export function providerCallbackType(
  signatureId: string,
  parameterName: string,
  parameters: readonly { readonly name: string; readonly type: ProviderTypeExpr }[],
): ProviderTypeExpr {
  return {
    kind: "function",
    id: `${signatureId}::parameter:${parameterName}`,
    parameters,
    returnType: voidType,
  };
}

export function fnExport(moduleSpecifier: string, name: string, parameters: readonly { name: string; type: ProviderTypeExpr; rest?: boolean }[], returnType: ProviderTypeExpr) {
  return {
    id: `${moduleSpecifier}::${name}`,
    name,
    kind: "function" as const,
    signatures: [{
      id: `${moduleSpecifier}::${name}(${parameters.map((parameter) => parameter.name).join(",")})`,
      name,
      parameters: parameters.map((parameter) => ({
        name: parameter.name,
        type: parameter.type,
        ...(parameter.rest === true ? { rest: true } : {}),
      })),
      returnType,
    }],
  };
}

export function valueExport(moduleSpecifier: string, name: string, type: ProviderTypeExpr) {
  return {
    id: `${moduleSpecifier}::${name}`,
    name,
    kind: "value" as const,
    type,
  };
}

export function methodMember(classId: string, name: string, parameters: readonly { name: string; type: ProviderTypeExpr }[], returnType: ProviderTypeExpr, options?: { readonly static?: boolean }) {
  return {
    id: `${classId}.${name}`,
    name,
    kind: "method" as const,
    ...(options?.static === true ? { static: true } : {}),
    signatures: [{
      id: `${classId}.${name}(${parameters.map((parameter) => parameter.name).join(",")})`,
      parameters: parameters.map((parameter) => ({ name: parameter.name, type: parameter.type })),
      returnType,
    }],
  };
}

export function propertyMember(
  classId: string,
  name: string,
  type: ProviderTypeExpr,
  options?: {
    readonly readonly?: boolean;
    readonly static?: boolean;
    readonly optional?: boolean;
  },
) {
  return {
    id: `${classId}.${name}`,
    name,
    kind: "property" as const,
    ...(options?.static === true ? { static: true } : {}),
    ...(options?.readonly === false ? {} : { readonly: true }),
    ...(options?.optional === true ? { optional: true } : {}),
    type,
  };
}

export function constructorMember(classId: string, parameters: readonly { name: string; type: ProviderTypeExpr }[]) {
  return {
    id: `${classId}.constructor`,
    name: "constructor",
    kind: "constructor" as const,
    signatures: [{
      id: `${classId}.constructor(${parameters.map((parameter) => parameter.name).join(",")})`,
      parameters: parameters.map((parameter) => ({ name: parameter.name, type: parameter.type })),
      returnType: voidType,
    }],
  };
}

// Declared exports without rows: selecting them diagnoses deterministically.
// Each carries documentation naming the contract it requires.
export function unsupportedFn(moduleSpecifier: string, name: string, requires: string) {
  return {
    id: `${moduleSpecifier}::${name}`,
    name,
    kind: "function" as const,
    documentation: `Unsupported: requires ${requires}.`,
    signatures: [{
      id: `${moduleSpecifier}::${name}(...)`,
      name,
      parameters: [{ name: "args", type: { kind: "array", elementType: { kind: "any" } as const } as const, rest: true }],
      returnType: { kind: "any" } as const,
    }],
  };
}
