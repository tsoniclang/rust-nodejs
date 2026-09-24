import {
  booleanType,
  boolCarrier,
  bufferCarrier,
  fnExport,
  noneArgument,
  numberType,
  int32Carrier,
  nativeUintCarrier,
  propertyMember,
  providerCallbackType,
  providerNativeFallibility,
  providerRef,
  rustOptionTargetType,
  undefinedType,
  unitCarrier,
  voidType,
  zlibCallbackCarrier,
  zlibOptionsCarrier,
  zlibTransformCarrier,
} from "../model.js";
import type {
  ProviderTypeExpr,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";
import { rustJsTypedArrayTargetType } from "@tsonic/target-rust/provider";

const moduleSpecifier = "node:zlib";
const optionsId = `${moduleSpecifier}::ZlibOptions`;
const transformId = `${moduleSpecifier}::Zlib`;
const bufferType = providerRef("node:buffer", "Buffer");
const optionsType = providerRef(moduleSpecifier, "ZlibOptions");
const optionalBufferType = {
  kind: "union",
  types: [bufferType, undefinedType],
} as const;
const callbackType = (signatureId: string): ProviderTypeExpr =>
  providerCallbackType(signatureId, "callback", [
    { name: "error", type: { kind: "any" } },
    { name: "result", type: bufferType },
  ]);

const syncOperations = [
  ["gzipSync", "gzip_sync", "gzip_sync_source"],
  ["gunzipSync", "gunzip_sync", "gunzip_sync_source"],
  ["deflateSync", "deflate_sync", "deflate_sync_source"],
  ["inflateSync", "inflate_sync", "inflate_sync_source"],
  ["deflateRawSync", "deflate_raw_sync", "deflate_raw_sync_source"],
  ["inflateRawSync", "inflate_raw_sync", "inflate_raw_sync_source"],
] as const;

const callbackOperations = [
  ["gzip", "gzip_callable", "gzip_options_callable"],
  ["gunzip", "gunzip_callable", "gunzip_options_callable"],
  ["deflate", "deflate_callable", "deflate_options_callable"],
  ["inflate", "inflate_callable", "inflate_options_callable"],
] as const;

const factories = [
  ["createGzip", "create_gzip", "create_gzip_source"],
  ["createGunzip", "create_gunzip", "create_gunzip_source"],
  ["createDeflate", "create_deflate", "create_deflate_source"],
  ["createInflate", "create_inflate", "create_inflate_source"],
  ["createDeflateRaw", "create_deflate_raw", "create_deflate_raw_source"],
  ["createInflateRaw", "create_inflate_raw", "create_inflate_raw_source"],
] as const;

export function zlibModule(typedArrays: boolean): RustProviderModuleDefinition {
  const byteInputType: ProviderTypeExpr = typedArrays ? { kind: "source-global", name: "Uint8Array" } : bufferType;
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.zlib",
    imports: [{
      moduleSpecifier: "node:buffer",
      namedImports: [{ exportedName: "Buffer" }],
    }],
    exports: [
      {
        id: optionsId,
        name: "ZlibOptions",
        kind: "interface",
        members: [
          ...(["flush", "finishFlush", "chunkSize", "windowBits", "level", "memLevel", "strategy", "maxOutputLength"] as const)
            .map((name) => propertyMember(optionsId, name, numberType, {
              readonly: false,
              optional: true,
            })),
          propertyMember(optionsId, "dictionary", bufferType, {
            readonly: false,
            optional: true,
          }),
          propertyMember(optionsId, "info", booleanType, {
            readonly: false,
            optional: true,
          }),
        ],
      },
      {
        id: transformId,
        name: "Zlib",
        kind: "class",
        members: [
          {
            id: `${transformId}.write`,
            name: "write",
            kind: "method",
            signatures: [{
              id: `${transformId}.write(input)`,
              parameters: [{ name: "input", type: bufferType }],
              returnType: booleanType,
            }],
          },
          {
            id: `${transformId}.read`,
            name: "read",
            kind: "method",
            signatures: [{
              id: `${transformId}.read()`,
              parameters: [],
              returnType: optionalBufferType,
            }],
          },
          {
            id: `${transformId}.end`,
            name: "end",
            kind: "method",
            signatures: [{
              id: `${transformId}.end()`,
              parameters: [],
              returnType: voidType,
            }],
          },
        ],
      },
      ...syncOperations.map(([name]) => ({
        id: `${moduleSpecifier}::${name}`,
        name,
        kind: "function" as const,
        signatures: [
          {
            id: `${moduleSpecifier}::${name}(input)`,
            name,
            parameters: [{ name: "input", type: byteInputType }],
            returnType: bufferType,
          },
          {
            id: `${moduleSpecifier}::${name}(input,options)`,
            name,
            parameters: [
              { name: "input", type: byteInputType },
              { name: "options", type: optionsType },
            ],
            returnType: bufferType,
          },
        ],
      })),
      ...callbackOperations.map(([name]) => ({
        id: `${moduleSpecifier}::${name}`,
        name,
        kind: "function" as const,
        signatures: [
          {
            id: `${moduleSpecifier}::${name}(input,callback)`,
            name,
            parameters: [
              { name: "input", type: bufferType },
              { name: "callback", type: callbackType(`${moduleSpecifier}::${name}(input,callback)`) },
            ],
            returnType: voidType,
          },
          {
            id: `${moduleSpecifier}::${name}(input,options,callback)`,
            name,
            parameters: [
              { name: "input", type: bufferType },
              { name: "options", type: optionsType },
              { name: "callback", type: callbackType(`${moduleSpecifier}::${name}(input,options,callback)`) },
            ],
            returnType: voidType,
          },
        ],
      })),
      ...factories.map(([name]) => ({
        id: `${moduleSpecifier}::${name}`,
        name,
        kind: "function" as const,
        signatures: [
          {
            id: `${moduleSpecifier}::${name}()`,
            name,
            parameters: [],
            returnType: providerRef(moduleSpecifier, "Zlib"),
          },
          {
            id: `${moduleSpecifier}::${name}(options)`,
            name,
            parameters: [{ name: "options", type: optionsType }],
            returnType: providerRef(moduleSpecifier, "Zlib"),
          },
        ],
      })),
      fnExport(moduleSpecifier, "brotliCompressSync", [{ name: "input", type: byteInputType }], bufferType),
      fnExport(moduleSpecifier, "brotliDecompressSync", [{ name: "input", type: byteInputType }], bufferType),
    ],
  };
}

export function zlibRows(typedArrays: boolean): readonly RustProviderOperationDefinition[] {
  const inputCarrier = typedArrays ? rustJsTypedArrayTargetType("Uint8Array") : bufferCarrier;
  const rows: RustProviderOperationDefinition[] = [];
  for (const [name, basePath, optionsPath] of syncOperations) {
    rows.push(
      {
        exportId: `${moduleSpecifier}::${name}`,
        signatureId: `${moduleSpecifier}::${name}(input)`,
        operationKind: "method",
        target: { form: "call", path: `node_zlib::${basePath}`, argModes: ["ref"] },
        resultCarrier: bufferCarrier,
        parameterCarriers: [inputCarrier],
        ...providerNativeFallibility,
      },
      {
        exportId: `${moduleSpecifier}::${name}`,
        signatureId: `${moduleSpecifier}::${name}(input,options)`,
        operationKind: "method",
        target: { form: "call", path: `node_zlib::${optionsPath}`, argModes: ["ref", "value"] },
        resultCarrier: bufferCarrier,
        parameterCarriers: [inputCarrier, zlibOptionsCarrier],
        ...providerNativeFallibility,
      },
    );
  }
  for (const [name, basePath, optionsPath] of callbackOperations) {
    rows.push(
      {
        exportId: `${moduleSpecifier}::${name}`,
        signatureId: `${moduleSpecifier}::${name}(input,callback)`,
        operationKind: "method",
        target: { form: "call", path: `node_zlib::${basePath}`, argModes: ["ref", "value"] },
        resultCarrier: unitCarrier,
        parameterCarriers: [bufferCarrier, zlibCallbackCarrier],
        ...providerNativeFallibility,
      },
      {
        exportId: `${moduleSpecifier}::${name}`,
        signatureId: `${moduleSpecifier}::${name}(input,options,callback)`,
        operationKind: "method",
        target: { form: "call", path: `node_zlib::${optionsPath}`, argModes: ["ref", "value", "value"] },
        resultCarrier: unitCarrier,
        parameterCarriers: [bufferCarrier, zlibOptionsCarrier, zlibCallbackCarrier],
        ...providerNativeFallibility,
      },
    );
  }
  for (const [name, basePath, optionsPath] of factories) {
    rows.push(
      {
        exportId: `${moduleSpecifier}::${name}`,
        signatureId: `${moduleSpecifier}::${name}()`,
        operationKind: "method",
        target: { form: "call", path: `node_zlib::${basePath}`, trailingArguments: [noneArgument] },
        resultCarrier: zlibTransformCarrier,
        parameterCarriers: [],
      },
      {
        exportId: `${moduleSpecifier}::${name}`,
        signatureId: `${moduleSpecifier}::${name}(options)`,
        operationKind: "method",
        target: { form: "call", path: `node_zlib::${optionsPath}`, argModes: ["value"] },
        resultCarrier: zlibTransformCarrier,
        parameterCarriers: [zlibOptionsCarrier],
        ...providerNativeFallibility,
      },
    );
  }
  rows.push(
    {
      exportId: `${moduleSpecifier}::brotliCompressSync`,
      operationKind: "method",
      target: { form: "call", path: "node_zlib::brotli_compress_sync", argModes: ["ref"] },
      resultCarrier: bufferCarrier,
      parameterCarriers: [inputCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: `${moduleSpecifier}::brotliDecompressSync`,
      operationKind: "method",
      target: { form: "call", path: "node_zlib::brotli_decompress_sync", argModes: ["ref"] },
      resultCarrier: bufferCarrier,
      parameterCarriers: [inputCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: transformId,
      memberId: `${transformId}.write`,
      operationKind: "method",
      target: { form: "receiver-method", name: "write", argModes: ["value"], mutatesReceiver: true },
      resultCarrier: boolCarrier,
      receiverCarrier: zlibTransformCarrier,
      parameterCarriers: [bufferCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: transformId,
      memberId: `${transformId}.read`,
      operationKind: "method",
      target: { form: "receiver-method", name: "read", mutatesReceiver: true },
      resultCarrier: rustOptionTargetType(bufferCarrier),
      receiverCarrier: zlibTransformCarrier,
      parameterCarriers: [],
    },
    {
      exportId: transformId,
      memberId: `${transformId}.end`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end", mutatesReceiver: true },
      resultCarrier: unitCarrier,
      receiverCarrier: zlibTransformCarrier,
      parameterCarriers: [],
      ...providerNativeFallibility,
    },
  );
  const optionFields = [
    ["flush", "flush", int32Carrier],
    ["finishFlush", "finish_flush", int32Carrier],
    ["chunkSize", "chunk_size", nativeUintCarrier],
    ["windowBits", "window_bits", int32Carrier],
    ["level", "level", int32Carrier],
    ["memLevel", "mem_level", int32Carrier],
    ["strategy", "strategy", int32Carrier],
    ["maxOutputLength", "max_output_length", nativeUintCarrier],
  ] as const;
  for (const [memberName, fieldName, carrier] of optionFields) {
    rows.push(
      {
        exportId: optionsId,
        memberId: `${optionsId}.${memberName}`,
        operationKind: "property",
        target: { form: "field", name: fieldName },
        resultCarrier: rustOptionTargetType(carrier),
        receiverCarrier: zlibOptionsCarrier,
      },
      {
        exportId: optionsId,
        memberId: `${optionsId}.${memberName}`,
        operationKind: "property-set",
        target: { form: "field", name: fieldName },
        resultCarrier: unitCarrier,
        parameterCarriers: [rustOptionTargetType(carrier)],
        receiverCarrier: zlibOptionsCarrier,
      },
    );
  }
  for (const operationKind of ["property", "property-set"] as const) {
    rows.push({
      exportId: optionsId,
      memberId: `${optionsId}.dictionary`,
      operationKind,
      target: { form: "field", name: "dictionary" },
      resultCarrier: operationKind === "property"
        ? rustOptionTargetType(bufferCarrier)
        : unitCarrier,
      ...(operationKind === "property-set"
        ? { parameterCarriers: [rustOptionTargetType(bufferCarrier)] }
        : {}),
      receiverCarrier: zlibOptionsCarrier,
    });
    rows.push({
      exportId: optionsId,
      memberId: `${optionsId}.info`,
      operationKind,
      target: { form: "field", name: "info" },
      resultCarrier: operationKind === "property"
        ? rustOptionTargetType(boolCarrier)
        : unitCarrier,
      ...(operationKind === "property-set"
        ? { parameterCarriers: [rustOptionTargetType(boolCarrier)] }
        : {}),
      receiverCarrier: zlibOptionsCarrier,
    });
  }
  return rows;
}
