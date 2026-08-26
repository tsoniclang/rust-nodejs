import {
  rustBorrowedStrToStringValueConversion,
  rustCallableTargetType,
  rustCloneTrait,
  rustCopyTrait,
  rustDefaultTrait,
  rustInt32ToUsizeValueConversion,
  rustJsArrayTargetType,
  rustJsValueTargetType,
  rustOptionTargetType,
  rustProviderPathTargetType,
  rustProviderTypeIdentity,
  rustSourcePrimitiveTargetType,
  rustStringTargetType,
  rustUnitTargetType,
  rustUint32ToInt32ValueConversion,
  rustUint64ToFloat64ValueConversion,
  rustUsizeToInt32ValueConversion,
} from "@tsonic/target-rust/provider";
import type {
  RustProviderConstantArgument,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustNamedTypeTraitContractEntry,
  RustTargetTypeRef,
  RustTraitImplementationEvidence,
} from "@tsonic/target-rust/provider";

export {
  rustBorrowedStrToStringValueConversion,
  rustCallableTargetType,
  rustInt32ToUsizeValueConversion,
  rustJsArrayTargetType,
  rustOptionTargetType,
  rustSourcePrimitiveTargetType,
  rustStringTargetType,
  rustUint32ToInt32ValueConversion,
  rustUint64ToFloat64ValueConversion,
  rustUsizeToInt32ValueConversion,
};
export type {
  RustProviderConstantArgument,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustTargetTypeRef,
};
export const stringCarrier: RustTargetTypeRef = rustStringTargetType();
export const boolCarrier: RustTargetTypeRef = rustSourcePrimitiveTargetType("bool");
export const int32Carrier: RustTargetTypeRef = rustSourcePrimitiveTargetType("int32");
export const float64Carrier: RustTargetTypeRef = rustSourcePrimitiveTargetType("float64");
export const jsValueCarrier: RustTargetTypeRef = rustJsValueTargetType();
export const stringArrayCarrier: RustTargetTypeRef = rustJsArrayTargetType(stringCarrier);
export const numberArrayCarrier: RustTargetTypeRef = rustJsArrayTargetType(float64Carrier);
const providerOwner = {
  packageId: "@tsonic/rust-nodejs",
  packageVersion: "0.0.1",
  compilationSnapshotId: "@tsonic/rust-nodejs@0.0.1",
} as const;
const cloneTraits = [{
  trait: rustCloneTrait,
  genericBindings: [],
  requirements: [],
}] as const;
const cloneCopyTraits = [
  { trait: rustCloneTrait, genericBindings: [], requirements: [] },
  { trait: rustCopyTrait, genericBindings: [], requirements: [] },
] as const;
const cloneCopyDefaultTraits = [
  { trait: rustCloneTrait, genericBindings: [], requirements: [] },
  { trait: rustDefaultTrait, genericBindings: [], requirements: [] },
  { trait: rustCopyTrait, genericBindings: [], requirements: [] },
] as const;
const cloneDefaultTraits = [
  { trait: rustCloneTrait, genericBindings: [], requirements: [] },
  { trait: rustDefaultTrait, genericBindings: [], requirements: [] },
] as const;

function nodeCarrier(
  itemId: string,
  displayPath: string,
): RustTargetTypeRef {
  return rustProviderPathTargetType({
    owner: providerOwner,
    itemId,
    displayPath,
  });
}

function nodeTraitContract(
  itemId: string,
  implementations: readonly RustTraitImplementationEvidence[] = cloneTraits,
): RustNamedTypeTraitContractEntry {
  return {
    typeIdentity: rustProviderTypeIdentity(providerOwner, itemId),
    contract: { implementations },
  };
}

export const nodeTraitContracts: readonly RustNamedTypeTraitContractEntry[] = [
  nodeTraitContract("rust.node.Buffer"),
  nodeTraitContract("rust.node.Hash"),
  nodeTraitContract("rust.node.Hmac"),
  nodeTraitContract("rust.node.HttpIncomingMessage"),
  nodeTraitContract("rust.node.HttpServer"),
  nodeTraitContract("rust.node.HttpServerResponse"),
  nodeTraitContract("rust.node.MakeDirectoryOptions", cloneCopyDefaultTraits),
  nodeTraitContract("rust.node.MemoryUsage"),
  nodeTraitContract("rust.node.NodeError"),
  nodeTraitContract("rust.node.ProcessEnv", cloneCopyDefaultTraits),
  nodeTraitContract("rust.node.ProcessWriteStream", cloneCopyTraits),
  nodeTraitContract("rust.node.RmOptions", cloneCopyDefaultTraits),
  nodeTraitContract("rust.node.SpawnSyncResult"),
  nodeTraitContract("rust.node.Stats"),
  nodeTraitContract("rust.node.TextDecoder"),
  nodeTraitContract("rust.node.Timeout"),
  nodeTraitContract("rust.node.Url"),
  nodeTraitContract("rust.node.UrlObject"),
  nodeTraitContract("rust.node.UrlSearchParams", cloneDefaultTraits),
];

export const statsCarrier: RustTargetTypeRef = nodeCarrier("rust.node.Stats", "tsonic_rust_node::fs::Stats");
export const makeDirectoryOptionsCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.MakeDirectoryOptions",
  "tsonic_rust_node::fs::MakeDirectoryOptions",
);
export const rmOptionsCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.RmOptions",
  "tsonic_rust_node::fs::RmOptions",
);
export const processEnvCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.ProcessEnv",
  "tsonic_rust_node::process::ProcessEnv",
);
export const processMemoryUsageCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.MemoryUsage",
  "tsonic_rust_node::process::MemoryUsage",
);
export const processWriteStreamCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.ProcessWriteStream",
  "tsonic_rust_node::process::ProcessWriteStream",
);
export const bufferCarrier: RustTargetTypeRef = nodeCarrier("rust.node.Buffer", "tsonic_rust_node::buffer::Buffer");
export const spawnSyncResultCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.SpawnSyncResult",
  "tsonic_rust_node::child_process::SpawnSyncResult",
);
export const urlCarrier: RustTargetTypeRef = nodeCarrier("rust.node.Url", "tsonic_rust_node::url::Url");
export const urlObjectCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.UrlObject",
  "tsonic_rust_node::url::LegacyUrlObject",
);
export const searchParamsCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.UrlSearchParams",
  "tsonic_rust_node::url::UrlSearchParams",
);
export const hashCarrier: RustTargetTypeRef = nodeCarrier("rust.node.Hash", "tsonic_rust_node::crypto::Hash");
export const hmacCarrier: RustTargetTypeRef = nodeCarrier("rust.node.Hmac", "tsonic_rust_node::crypto::Hmac");
export const httpIncomingMessageCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.HttpIncomingMessage",
  "tsonic_rust_node::http::IncomingMessage",
);
export const httpServerResponseCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.HttpServerResponse",
  "tsonic_rust_node::http::ServerResponseHandle",
);
export const httpServerCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.HttpServer",
  "tsonic_rust_node::http::ServerHandle",
);
export const timeoutCarrier: RustTargetTypeRef = nodeCarrier("rust.node.Timeout", "tsonic_rust_node::timers::Timeout");
export const textDecoderCarrier: RustTargetTypeRef = nodeCarrier(
  "rust.node.TextDecoder",
  "tsonic_rust_node::util::TextDecoder",
);
export const nodeErrorCarrier: RustTargetTypeRef = nodeCarrier("rust.node.NodeError", "tsonic_rust_node::NodeError");
export const unitCarrier: RustTargetTypeRef = rustUnitTargetType();
export const emptyCallbackCarrier: RustTargetTypeRef = rustCallableTargetType([], unitCarrier);
export const httpRequestCallbackCarrier: RustTargetTypeRef = rustCallableTargetType(
  [httpIncomingMessageCarrier, httpServerResponseCarrier],
  unitCarrier,
);
export const trueArgument = { kind: "boolean", value: true } as const;
export const noneArgument = { kind: "none" } as const;
export const zeroFloat64Argument = { kind: "float64", value: 0 } as const;
export const providerNativeFallibility: {
  readonly isFallible: true;
  readonly errorBoundary: "provider-native";
  readonly errorCarrier: RustTargetTypeRef;
} = {
  isFallible: true,
  errorBoundary: "provider-native",
  errorCarrier: nodeErrorCarrier,
} as const;
export const stringType = { kind: "string" } as const;
export const numberType = { kind: "number" } as const;
export const booleanType = { kind: "boolean" } as const;
export const voidType = { kind: "void" } as const;
export const int32Type = { kind: "source-primitive", name: "int32" } as const;
export const stringArrayType = { kind: "array", elementType: stringType } as const;
export const numberArrayType = { kind: "array", elementType: numberType } as const;

export const nullType = { kind: "literal", value: null } as const;
export const undefinedType = { kind: "undefined" } as const;

export type ProviderTypeExpr =
  | typeof stringType
  | typeof numberType
  | typeof booleanType
  | typeof voidType
  | typeof int32Type
  | typeof stringArrayType
  | typeof numberArrayType
  | typeof undefinedType
  | {
      readonly kind: "provider-ref";
      readonly moduleSpecifier: string;
      readonly exportName: string;
      readonly typeArguments?: readonly ProviderTypeExpr[];
    }
  | { readonly kind: "type-parameter"; readonly name: string }
  | { readonly kind: "array"; readonly elementType: ProviderTypeExpr }
  | { readonly kind: "union"; readonly types: readonly ProviderTypeExpr[] }
  | typeof nullType
  | { readonly kind: "any" }
  | {
      readonly kind: "function";
      readonly id: string;
      readonly parameters: readonly { readonly name: string; readonly type: ProviderTypeExpr }[];
      readonly returnType: ProviderTypeExpr;
    };

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

// --- node:assert -------------------------------------------------------------
