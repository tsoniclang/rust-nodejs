import {
  rustBorrowedStrToStringValueConversion,
  rustCallableTargetType,
  rustInt32ToUsizeValueConversion,
  rustJsArrayTargetType,
  rustJsPromiseTargetType,
  rustOptionTargetType,
  rustSourcePrimitiveTargetType,
  rustStringTargetType,
  rustUint32ToInt32ValueConversion,
  rustUint64ToFloat64ValueConversion,
  rustUsizeToInt32ValueConversion,
} from "@tsonic/target-rust/provider";
import type {
  RustProviderConstantArgument,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustTargetTypeRef,
} from "@tsonic/target-rust/provider";

export {
  rustBorrowedStrToStringValueConversion,
  rustCallableTargetType,
  rustInt32ToUsizeValueConversion,
  rustJsArrayTargetType,
  rustJsPromiseTargetType,
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
export const stringCarrier = rustStringTargetType();
export const boolCarrier = rustSourcePrimitiveTargetType("bool");
export const int32Carrier = rustSourcePrimitiveTargetType("int32");
export const float64Carrier = rustSourcePrimitiveTargetType("float64");
export const jsValueCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.js.JsValue" };
export const stringArrayCarrier = rustJsArrayTargetType(stringCarrier);
export const numberArrayCarrier = rustJsArrayTargetType(float64Carrier);
export const statsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Stats" };
export const makeDirectoryOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.MakeDirectoryOptions" };
export const rmOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.RmOptions" };
export const processEnvCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ProcessEnv" };
export const processMemoryUsageCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.MemoryUsage" };
export const bufferCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Buffer" };
export const spawnSyncResultCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.SpawnSyncResult" };
export const urlCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Url" };
export const urlObjectCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.UrlObject" };
export const searchParamsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.UrlSearchParams" };
export const hashCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Hash" };
export const hmacCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Hmac" };
export const httpIncomingMessageCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpIncomingMessage" };
export const httpServerResponseCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpServerResponse" };
export const httpServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpServer" };
export const timeoutCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Timeout" };
export const textDecoderCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.TextDecoder" };
export const eventEmitterCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.EventEmitter" };
export const readableCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Readable" };
export const writableCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Writable" };
export const readStreamCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ReadStream" };
export const writeStreamCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.WriteStream" };
export const readStreamOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ReadStreamOptions" };
export const writeStreamOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.WriteStreamOptions" };
export const fsWatcherCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.FsWatcher" };
export const dnsLookupAddressCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.DnsLookupAddress" };
export const zlibOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ZlibOptions" };
export const zlibTransformCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ZlibTransform" };
export const netSocketCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.NetSocket" };
export const netServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.NetServer" };
export const tlsConnectOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.TlsConnectOptions" };
export const tlsServerOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.TlsServerOptions" };
export const tlsSocketCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.TlsSocket" };
export const tlsServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.TlsServer" };
export const httpsServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpsServer" };
export const httpsClientRequestCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpsClientRequest" };
export const readlineOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ReadlineOptions" };
export const readlineInterfaceCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ReadlineInterface" };
export const workerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Worker" };
export const workerOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.WorkerOptions" };
export const messagePortCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.MessagePort" };
export const messageChannelCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.MessageChannel" };
export const nodeErrorCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.NodeError" };
export const unitCarrier: RustTargetTypeRef = { kind: "tuple", elements: [] };
export const emptyCallbackCarrier = rustCallableTargetType([], unitCarrier);
export const oneValueCallbackCarrier = rustCallableTargetType([jsValueCarrier], unitCarrier);
export const twoValueCallbackCarrier = rustCallableTargetType(
  [jsValueCarrier, jsValueCarrier],
  unitCarrier,
);
export const threeValueCallbackCarrier = rustCallableTargetType(
  [jsValueCarrier, jsValueCarrier, jsValueCarrier],
  unitCarrier,
);
export const fileWatchCallbackCarrier = rustCallableTargetType(
  [stringCarrier, stringCarrier],
  unitCarrier,
);
export const fileStatWatchCallbackCarrier = rustCallableTargetType(
  [statsCarrier, statsCarrier],
  unitCarrier,
);
export const dnsLookupCallbackCarrier = rustCallableTargetType(
  [rustOptionTargetType(nodeErrorCarrier), stringCarrier, float64Carrier],
  unitCarrier,
);
export const dnsAddressArrayCallbackCarrier = rustCallableTargetType(
  [rustOptionTargetType(nodeErrorCarrier), stringArrayCarrier],
  unitCarrier,
);
export const zlibCallbackCarrier = rustCallableTargetType(
  [rustOptionTargetType(nodeErrorCarrier), bufferCarrier],
  unitCarrier,
);
export const netConnectionCallbackCarrier = rustCallableTargetType(
  [netSocketCarrier],
  unitCarrier,
);
export const tlsSocketCallbackCarrier = rustCallableTargetType(
  [tlsSocketCarrier],
  unitCarrier,
);
export const readlineQuestionCallbackCarrier = rustCallableTargetType(
  [stringCarrier],
  unitCarrier,
);
export const mutableEventEmitterCarrier: RustTargetTypeRef = {
  kind: "reference",
  referent: eventEmitterCarrier,
  mutable: true,
};
export const httpRequestCallbackCarrier = rustCallableTargetType(
  [httpIncomingMessageCarrier, httpServerResponseCarrier],
  unitCarrier,
);
export const httpResponseCallbackCarrier = rustCallableTargetType(
  [httpIncomingMessageCarrier],
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
};
export const cloneOnlyCarrierTraits = {
  implementations: [{ traitPath: "core::clone::Clone", requirements: [] }],
} as const;
export const closedJsValueCarrierTraits = {
  implementations: [
    { traitPath: "core::clone::Clone", requirements: [] },
    {
      traitPath: "tsonic_rust_js::value::JsClosedValueCarrier",
      requirements: [],
    },
  ],
} as const;
export const copyDefaultCarrierTraits = {
  implementations: [
    { traitPath: "core::clone::Clone", requirements: [] },
    { traitPath: "core::default::Default", requirements: [] },
    { traitPath: "core::marker::Copy", requirements: [] },
  ],
} as const;
export const cloneDefaultCarrierTraits = {
  implementations: [
    { traitPath: "core::clone::Clone", requirements: [] },
    { traitPath: "core::default::Default", requirements: [] },
  ],
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
  | {
      readonly kind: "source-global";
      readonly name: string;
      readonly typeArguments?: readonly ProviderTypeExpr[];
    }
  | { readonly kind: "type-parameter"; readonly name: string }
  | { readonly kind: "array"; readonly elementType: ProviderTypeExpr }
  | { readonly kind: "union"; readonly types: readonly ProviderTypeExpr[] }
  | { readonly kind: "literal"; readonly value: string | number | boolean | null }
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

// --- node:assert -------------------------------------------------------------
