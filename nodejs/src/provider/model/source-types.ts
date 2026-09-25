import type { RustProviderModuleDefinition } from "@tsonic/target-rust/provider";

export const stringType = { kind: "string" } as const;

export const numberType = { kind: "number" } as const;

export const booleanType = { kind: "boolean" } as const;

export const voidType = { kind: "void" } as const;

export const int32Type = { kind: "source-primitive", name: "int32" } as const;

export const stringArrayType = { kind: "array", elementType: stringType } as const;

export const numberArrayType = { kind: "array", elementType: numberType } as const;

export const nullType = { kind: "literal", value: null } as const;

export const undefinedType = { kind: "undefined" } as const;

export type ProviderTypeExpr = NonNullable<RustProviderModuleDefinition["exports"][number]["type"]>;
