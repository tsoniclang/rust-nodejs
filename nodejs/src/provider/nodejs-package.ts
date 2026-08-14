import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  createRustProviderPackage,
  rustCallableTargetType,
  rustInt32ToUint8ValueConversion,
  rustInt32ToUsizeValueConversion,
  rustJsArrayTargetType,
  rustOptionTargetType,
  rustSourcePrimitiveTargetType,
  rustStringTargetType,
  rustUint32ToInt32ValueConversion,
  rustUint64ToFloat64ValueConversion,
  rustUint8ToInt32ValueConversion,
  rustUsizeToInt32ValueConversion,
} from "@tsonic/target-rust";
import type {
  RustProviderConstantArgument,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustProviderPackageImplementation,
  RustTargetTypeRef,
} from "@tsonic/target-rust";

// Compiled layout is dist/provider/nodejs-package.js, so the installed
// package root is two directories up from this module.
const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

const stringCarrier = rustStringTargetType();
const boolCarrier = rustSourcePrimitiveTargetType("bool");
const int32Carrier = rustSourcePrimitiveTargetType("int32");
const float64Carrier = rustSourcePrimitiveTargetType("float64");
const jsValueCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.js.JsValue" };
const stringArrayCarrier = rustJsArrayTargetType(stringCarrier);
const statsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Stats" };
const processEnvCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ProcessEnv" };
const bufferCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Buffer" };
const urlCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Url" };
const urlObjectCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.UrlObject" };
const searchParamsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.UrlSearchParams" };
const hashCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Hash" };
const hmacCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Hmac" };
const httpIncomingMessageCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpIncomingMessage" };
const httpServerResponseCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpServerResponse" };
const httpServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpServer" };
const timeoutCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Timeout" };
const unitCarrier: RustTargetTypeRef = { kind: "tuple", elements: [] };
const emptyCallbackCarrier = rustCallableTargetType([], unitCarrier);
const httpRequestCallbackCarrier = rustCallableTargetType(
  [httpIncomingMessageCarrier, httpServerResponseCarrier],
  unitCarrier,
);
const trueArgument = { kind: "boolean", value: true } as const;
const noneArgument = { kind: "none" } as const;
const toStringStep = { kind: "method", name: "to_string" } as const;
const providerNativeFallibility = {
  isFallible: true,
  errorBoundary: "provider-native",
} as const;

const stringType = { kind: "string" } as const;
const numberType = { kind: "number" } as const;
const booleanType = { kind: "boolean" } as const;
const voidType = { kind: "void" } as const;
const int32Type = { kind: "source-primitive", name: "int32" } as const;
const stringArrayType = { kind: "array", elementType: stringType } as const;

const nullType = { kind: "literal", value: null } as const;
const undefinedType = { kind: "undefined" } as const;

type ProviderTypeExpr =
  | typeof stringType
  | typeof numberType
  | typeof booleanType
  | typeof voidType
  | typeof int32Type
  | typeof stringArrayType
  | typeof undefinedType
  | { readonly kind: "provider-ref"; readonly moduleSpecifier: string; readonly exportName: string }
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

function providerRef(moduleSpecifier: string, exportName: string): ProviderTypeExpr {
  return { kind: "provider-ref", moduleSpecifier, exportName };
}

function fnExport(moduleSpecifier: string, name: string, parameters: readonly { name: string; type: ProviderTypeExpr; rest?: boolean }[], returnType: ProviderTypeExpr) {
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

function valueExport(moduleSpecifier: string, name: string, type: ProviderTypeExpr) {
  return {
    id: `${moduleSpecifier}::${name}`,
    name,
    kind: "value" as const,
    type,
  };
}

function methodMember(classId: string, name: string, parameters: readonly { name: string; type: ProviderTypeExpr }[], returnType: ProviderTypeExpr, options?: { readonly static?: boolean }) {
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

function propertyMember(
  classId: string,
  name: string,
  type: ProviderTypeExpr,
  options?: { readonly readonly?: boolean; readonly static?: boolean },
) {
  return {
    id: `${classId}.${name}`,
    name,
    kind: "property" as const,
    ...(options?.static === true ? { static: true } : {}),
    ...(options?.readonly === false ? {} : { readonly: true }),
    type,
  };
}

// --- node:http ---------------------------------------------------------------

function httpModule(): RustProviderModuleDefinition {
  const m = "node:http";
  const incomingId = `${m}::IncomingMessage`;
  const responseId = `${m}::ServerResponse`;
  const serverId = `${m}::Server`;
  const emptyCallbackType = (id: string): ProviderTypeExpr => ({
    kind: "function",
    id,
    parameters: [],
    returnType: voidType,
  });
  const requestCallbackType: ProviderTypeExpr = {
    kind: "function",
    id: `${m}.RequestCallback`,
    parameters: [
      { name: "request", type: providerRef(m, "IncomingMessage") },
      { name: "response", type: providerRef(m, "ServerResponse") },
    ],
    returnType: voidType,
  };
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.http",
    imports: [{
      moduleSpecifier: "node:buffer",
      namedImports: [{ exportedName: "Buffer" }],
    }],
    exports: [
      {
        id: incomingId,
        name: "IncomingMessage",
        kind: "class",
        members: [propertyMember(incomingId, "url", stringType)],
      },
      {
        id: responseId,
        name: "ServerResponse",
        kind: "class",
        members: [
          propertyMember(responseId, "statusCode", int32Type, { readonly: false }),
          methodMember(responseId, "setHeader", [
            { name: "name", type: stringType },
            { name: "value", type: stringType },
          ], voidType),
          {
            id: `${responseId}.end`,
            name: "end",
            kind: "method",
            signatures: [
              { id: `${responseId}.end()`, parameters: [], returnType: voidType },
              {
                id: `${responseId}.end(string)`,
                parameters: [{ name: "chunk", type: stringType }],
                returnType: voidType,
              },
              {
                id: `${responseId}.end(buffer)`,
                parameters: [{ name: "chunk", type: providerRef("node:buffer", "Buffer") }],
                returnType: voidType,
              },
            ],
          },
        ],
      },
      {
        id: serverId,
        name: "Server",
        kind: "class",
        members: [{
          id: `${serverId}.listen`,
          name: "listen",
          kind: "method",
          signatures: [
            {
              id: `${serverId}.listen(port,callback)`,
              parameters: [
                { name: "port", type: int32Type },
                { name: "callback", type: emptyCallbackType(`${serverId}.listen(port,callback).callback`) },
              ],
              returnType: providerRef(m, "Server"),
            },
            {
              id: `${serverId}.listen(port,host,callback)`,
              parameters: [
                { name: "port", type: int32Type },
                { name: "host", type: stringType },
                { name: "callback", type: emptyCallbackType(`${serverId}.listen(port,host,callback).callback`) },
              ],
              returnType: providerRef(m, "Server"),
            },
          ],
        }],
      },
      fnExport(m, "createServer", [{ name: "handler", type: requestCallbackType }], providerRef(m, "Server")),
    ],
  };
}

function httpRows(): readonly RustProviderOperationDefinition[] {
  const m = "node:http";
  const incomingId = `${m}::IncomingMessage`;
  const responseId = `${m}::ServerResponse`;
  const serverId = `${m}::Server`;
  return [
    {
      exportId: `${m}::createServer`,
      operationKind: "method",
      target: { form: "call", path: "node_http::create_server_callable" },
      resultCarrier: httpServerCarrier,
      parameterCarriers: [httpRequestCallbackCarrier],
    },
    {
      exportId: incomingId,
      memberId: `${incomingId}.url`,
      operationKind: "property",
      target: { form: "receiver-method", name: "url" },
      resultCarrier: stringCarrier,
    },
    {
      exportId: responseId,
      memberId: `${responseId}.statusCode`,
      operationKind: "property",
      target: { form: "receiver-method", name: "status_code" },
      resultCarrier: int32Carrier,
    },
    {
      exportId: responseId,
      memberId: `${responseId}.statusCode`,
      operationKind: "property-set",
      target: { form: "receiver-method", name: "set_status_code" },
      resultCarrier: unitCarrier,
      parameterCarriers: [int32Carrier],
    },
    {
      exportId: responseId,
      memberId: `${responseId}.setHeader`,
      operationKind: "method",
      target: { form: "receiver-method", name: "set_header", argModes: ["ref", "ref"] },
      resultCarrier: unitCarrier,
      parameterCarriers: [stringCarrier, stringCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: responseId,
      memberId: `${responseId}.end`,
      signatureId: `${responseId}.end()`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end_empty" },
      resultCarrier: unitCarrier,
    },
    {
      exportId: responseId,
      memberId: `${responseId}.end`,
      signatureId: `${responseId}.end(string)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end_string", argModes: ["ref"] },
      resultCarrier: unitCarrier,
      parameterCarriers: [stringCarrier],
    },
    {
      exportId: responseId,
      memberId: `${responseId}.end`,
      signatureId: `${responseId}.end(buffer)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "end_buffer" },
      resultCarrier: unitCarrier,
      parameterCarriers: [bufferCarrier],
    },
    {
      exportId: serverId,
      memberId: `${serverId}.listen`,
      signatureId: `${serverId}.listen(port,callback)`,
      operationKind: "method",
      target: {
        form: "receiver-method",
        name: "listen_default_host",
        argModes: ["value", "value"],
      },
      resultCarrier: httpServerCarrier,
      parameterCarriers: [int32Carrier, emptyCallbackCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: serverId,
      memberId: `${serverId}.listen`,
      signatureId: `${serverId}.listen(port,host,callback)`,
      operationKind: "method",
      target: {
        form: "receiver-method",
        name: "listen",
        argModes: ["value", "ref", "value"],
      },
      resultCarrier: httpServerCarrier,
      parameterCarriers: [int32Carrier, stringCarrier, emptyCallbackCarrier],
      ...providerNativeFallibility,
    },
  ];
}

// --- node:timers -------------------------------------------------------------

function timersModule(): RustProviderModuleDefinition {
  const m = "node:timers";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.timers",
    exports: [
      {
        id: `${m}::Timeout`,
        name: "Timeout",
        kind: "class",
        members: [],
      },
      fnExport(m, "setInterval", [
        {
          name: "callback",
          type: {
            kind: "function",
            id: `${m}.IntervalCallback`,
            parameters: [],
            returnType: voidType,
          },
        },
        { name: "delay", type: int32Type },
      ], providerRef(m, "Timeout")),
    ],
  };
}

function timersRows(): readonly RustProviderOperationDefinition[] {
  return [{
    exportId: "node:timers::setInterval",
    operationKind: "method",
    target: { form: "call", path: "node_timers::set_interval_callable" },
    resultCarrier: timeoutCarrier,
    parameterCarriers: [emptyCallbackCarrier, int32Carrier],
  }];
}

function constructorMember(classId: string, parameters: readonly { name: string; type: ProviderTypeExpr }[]) {
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
function unsupportedFn(moduleSpecifier: string, name: string, requires: string) {
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

function assertModule(): RustProviderModuleDefinition {
  const moduleSpecifier = "node:assert";
  const exportId = `${moduleSpecifier}::ok`;
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.assert",
    exports: [{
      id: exportId,
      name: "ok",
      kind: "function" as const,
      signatures: [
        {
          id: `${exportId}(value)`,
          name: "ok",
          parameters: [{ name: "value", type: booleanType }],
          returnType: voidType,
        },
        {
          id: `${exportId}(value,message)`,
          name: "ok",
          parameters: [
            { name: "value", type: booleanType },
            { name: "message", type: stringType },
          ],
          returnType: voidType,
        },
      ],
    }],
  };
}

function assertRows(): readonly RustProviderOperationDefinition[] {
  const exportId = "node:assert::ok";
  return [
    {
      exportId,
      signatureId: `${exportId}(value)`,
      operationKind: "method",
      target: {
        form: "call",
        path: "node_assert::ok",
        trailingArguments: [noneArgument],
      },
      resultCarrier: { kind: "tuple", elements: [] },
      parameterCarriers: [boolCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId,
      signatureId: `${exportId}(value,message)`,
      operationKind: "method",
      target: {
        form: "call",
        path: "node_assert::ok_with_message",
        argModes: ["value", "ref"],
      },
      resultCarrier: { kind: "tuple", elements: [] },
      parameterCarriers: [boolCarrier, stringCarrier],
      ...providerNativeFallibility,
    },
  ];
}

// --- node:path ---------------------------------------------------------------

function pathModule(): RustProviderModuleDefinition {
  const m = "node:path";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.path",
    exports: [
      fnExport(m, "join", [{ name: "paths", type: stringArrayType, rest: true }], stringType),
      fnExport(m, "resolve", [{ name: "paths", type: stringArrayType, rest: true }], stringType),
      fnExport(m, "normalize", [{ name: "path", type: stringType }], stringType),
      fnExport(m, "dirname", [{ name: "path", type: stringType }], stringType),
      fnExport(m, "basename", [{ name: "path", type: stringType }], stringType),
      fnExport(m, "extname", [{ name: "path", type: stringType }], stringType),
      fnExport(m, "isAbsolute", [{ name: "path", type: stringType }], booleanType),
      fnExport(m, "relative", [{ name: "from", type: stringType }, { name: "to", type: stringType }], stringType),
      valueExport(m, "sep", stringType),
    ],
  };
}

function pathRows(): readonly RustProviderOperationDefinition[] {
  const simple = (name: string, rustPath: string, resultCarrier = stringCarrier): RustProviderOperationDefinition => ({
    exportId: `node:path::${name}`,
    operationKind: "method",
    target: { form: "call", path: rustPath, argModes: ["ref"] },
    resultCarrier,
    parameterCarriers: [stringCarrier],
  });
  return [
    { exportId: "node:path::join", operationKind: "method", target: { form: "call-str-slice", path: "node_path::join" }, resultCarrier: stringCarrier },
    { exportId: "node:path::resolve", operationKind: "method", target: { form: "call-str-slice", path: "node_path::resolve" }, resultCarrier: stringCarrier, ...providerNativeFallibility },
    simple("normalize", "node_path::normalize"),
    simple("dirname", "node_path::dirname"),
    simple("extname", "node_path::extname"),
    simple("isAbsolute", "node_path::is_absolute", boolCarrier),
    {
      exportId: "node:path::relative",
      operationKind: "method",
      target: { form: "call", path: "node_path::relative", argModes: ["ref", "ref"] },
      resultCarrier: stringCarrier,
      parameterCarriers: [stringCarrier, stringCarrier],
    },
    {
      exportId: "node:path::sep",
      operationKind: "property",
      target: { form: "call", path: "node_path::sep", chain: [toStringStep] },
      resultCarrier: stringCarrier,
    },
    {
      exportId: "node:path::basename",
      operationKind: "method",
      target: { form: "call", path: "node_path::basename", argModes: ["ref"], trailingArguments: [noneArgument] },
      resultCarrier: stringCarrier,
      parameterCarriers: [stringCarrier],
    },
  ];
}

// --- node:os -----------------------------------------------------------------

function osModule(): RustProviderModuleDefinition {
  const m = "node:os";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.os",
    exports: [
      fnExport(m, "platform", [], stringType),
      fnExport(m, "arch", [], stringType),
      fnExport(m, "eol", [], stringType),
      fnExport(m, "hostname", [], stringType),
      fnExport(m, "tmpdir", [], stringType),
      fnExport(m, "homedir", [], { kind: "union", types: [stringType, nullType] }),
    ],
  };
}

function osRows(): readonly RustProviderOperationDefinition[] {
  const call = (name: string, path: string): RustProviderOperationDefinition => ({
    exportId: `node:os::${name}`,
    operationKind: "method",
    target: { form: "call", path },
    resultCarrier: stringCarrier,
  });
  return [
    call("platform", "node_os::platform"),
    call("arch", "node_os::arch"),
    { exportId: "node:os::eol", operationKind: "method", target: { form: "call", path: "node_os::eol", chain: [toStringStep] }, resultCarrier: stringCarrier },
    call("hostname", "node_os::hostname"),
    {
      exportId: "node:os::tmpdir",
      operationKind: "method",
      target: { form: "call", path: "node_os::tmpdir" },
      resultCarrier: stringCarrier,
      ...providerNativeFallibility,
    },
    { exportId: "node:os::homedir", operationKind: "method", target: { form: "call", path: "node_os::homedir" }, resultCarrier: rustOptionTargetType(stringCarrier) },
  ];
}

// --- node:fs -----------------------------------------------------------------

function fsModule(): RustProviderModuleDefinition {
  const m = "node:fs";
  const statsId = "node:fs::Stats";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.fs",
    imports: [{ moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] }],
    exports: [
      fnExport(m, "existsSync", [{ name: "path", type: stringType }], booleanType),
      {
        id: `${m}::readFileSync`,
        name: "readFileSync",
        kind: "function" as const,
        signatures: [
          {
            id: `${m}::readFileSync(path)`,
            name: "readFileSync",
            parameters: [{ name: "path", type: stringType }],
            returnType: providerRef("node:buffer", "Buffer"),
          },
          {
            id: `${m}::readFileSync(path,encoding)`,
            name: "readFileSync",
            parameters: [{ name: "path", type: stringType }, { name: "encoding", type: stringType }],
            returnType: stringType,
          },
        ],
      },
      {
        id: `${m}::writeFileSync`,
        name: "writeFileSync",
        kind: "function" as const,
        signatures: [
          {
            id: `${m}::writeFileSync(path,data,encoding)`,
            name: "writeFileSync",
            parameters: [
              { name: "path", type: stringType },
              { name: "data", type: stringType },
              { name: "encoding", type: stringType },
            ],
            returnType: voidType,
          },
          {
            id: `${m}::writeFileSync(path,buffer)`,
            name: "writeFileSync",
            parameters: [
              { name: "path", type: stringType },
              { name: "data", type: providerRef("node:buffer", "Buffer") },
            ],
            returnType: voidType,
          },
        ],
      },
      fnExport(m, "readdirSync", [{ name: "path", type: stringType }], stringArrayType),
      fnExport(m, "statSync", [{ name: "path", type: stringType }], providerRef(m, "Stats")),
      fnExport(m, "lstatSync", [{ name: "path", type: stringType }], providerRef(m, "Stats")),
      {
        id: `${m}::mkdirSync`,
        name: "mkdirSync",
        kind: "function" as const,
        signatures: [
          { id: `${m}::mkdirSync(path)`, name: "mkdirSync", parameters: [{ name: "path", type: stringType }], returnType: voidType },
          { id: `${m}::mkdirSync(path,recursive)`, name: "mkdirSync", parameters: [{ name: "path", type: stringType }, { name: "recursive", type: booleanType }], returnType: voidType },
        ],
      },
      {
        id: `${m}::rmSync`,
        name: "rmSync",
        kind: "function" as const,
        signatures: [
          { id: `${m}::rmSync(path)`, name: "rmSync", parameters: [{ name: "path", type: stringType }], returnType: voidType },
          { id: `${m}::rmSync(path,recursive)`, name: "rmSync", parameters: [{ name: "path", type: stringType }, { name: "recursive", type: booleanType }], returnType: voidType },
        ],
      },
      fnExport(m, "mkdtempSync", [{ name: "prefix", type: stringType }], stringType),
      fnExport(m, "unlinkSync", [{ name: "path", type: stringType }], voidType),
      fnExport(m, "symlinkSync", [{ name: "target", type: stringType }, { name: "path", type: stringType }], voidType),
      fnExport(m, "copyFileSync", [{ name: "from", type: stringType }, { name: "to", type: stringType }], voidType),
      fnExport(m, "renameSync", [{ name: "from", type: stringType }, { name: "to", type: stringType }], voidType),
      fnExport(m, "realpathSync", [{ name: "path", type: stringType }], stringType),
      {
        id: statsId,
        name: "Stats",
        kind: "class" as const,
        members: [
          methodMember(statsId, "isFile", [], booleanType),
          methodMember(statsId, "isDirectory", [], booleanType),
          methodMember(statsId, "isSymbolicLink", [], booleanType),
          propertyMember(statsId, "size", numberType),
          propertyMember(statsId, "mtimeMs", numberType),
        ],
      },
      unsupportedFn(m, "watch", "a filesystem event subscription contract"),
      unsupportedFn(m, "createReadStream", "a stream carrier contract"),
      unsupportedFn(m, "createWriteStream", "a stream carrier contract"),
    ],
  };
}

function fsRows(): readonly RustProviderOperationDefinition[] {
  const statsId = "node:fs::Stats";
  const fallible = (name: string, path: string, resultCarrier: RustTargetTypeRef, parameterCarriers: readonly RustTargetTypeRef[], trailingArguments?: readonly RustProviderConstantArgument[]): RustProviderOperationDefinition => ({
    exportId: `node:fs::${name}`,
    operationKind: "method",
    target: { form: "call", path, argModes: parameterCarriers.map(() => "ref"), ...(trailingArguments === undefined ? {} : { trailingArguments }) },
    resultCarrier,
    parameterCarriers,
    ...providerNativeFallibility,
  });
  return [
    { exportId: "node:fs::existsSync", operationKind: "method", target: { form: "call", path: "node_fs::exists_sync", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [stringCarrier] },
    {
      ...fallible("readFileSync", "node_fs::read_file_sync_buffer", bufferCarrier, [stringCarrier]),
      signatureId: "node:fs::readFileSync(path)",
    },
    {
      ...fallible("readFileSync", "node_fs::read_file_sync_string", stringCarrier, [stringCarrier, stringCarrier]),
      signatureId: "node:fs::readFileSync(path,encoding)",
    },
    {
      ...fallible("writeFileSync", "node_fs::write_file_sync_string", { kind: "tuple", elements: [] }, [stringCarrier, stringCarrier, stringCarrier]),
      signatureId: "node:fs::writeFileSync(path,data,encoding)",
    },
    {
      ...fallible("writeFileSync", "node_fs::write_file_sync_buffer", { kind: "tuple", elements: [] }, [stringCarrier, bufferCarrier]),
      signatureId: "node:fs::writeFileSync(path,buffer)",
    },
    fallible("readdirSync", "node_fs::readdir_sync", stringArrayCarrier, [stringCarrier]),
    fallible("statSync", "node_fs::stat_sync", statsCarrier, [stringCarrier]),
    fallible("lstatSync", "node_fs::lstat_sync", statsCarrier, [stringCarrier]),
    {
      ...fallible("mkdirSync", "node_fs::mkdir_sync", { kind: "tuple", elements: [] }, [stringCarrier], [{ kind: "boolean", value: false }]),
      signatureId: "node:fs::mkdirSync(path)",
    },
    {
      exportId: "node:fs::mkdirSync",
      signatureId: "node:fs::mkdirSync(path,recursive)",
      operationKind: "method",
      target: { form: "call", path: "node_fs::mkdir_sync", argModes: ["ref", "value"] },
      resultCarrier: { kind: "tuple", elements: [] },
      parameterCarriers: [stringCarrier, boolCarrier],
      ...providerNativeFallibility,
    },
    {
      ...fallible("rmSync", "node_fs::rm_sync", { kind: "tuple", elements: [] }, [stringCarrier], [{ kind: "boolean", value: false }, trueArgument]),
      signatureId: "node:fs::rmSync(path)",
    },
    {
      exportId: "node:fs::rmSync",
      signatureId: "node:fs::rmSync(path,recursive)",
      operationKind: "method",
      target: { form: "call", path: "node_fs::rm_sync", argModes: ["ref", "value"], trailingArguments: [trueArgument] },
      resultCarrier: { kind: "tuple", elements: [] },
      parameterCarriers: [stringCarrier, boolCarrier],
      ...providerNativeFallibility,
    },
    fallible("mkdtempSync", "node_fs::mkdtemp_sync", stringCarrier, [stringCarrier]),
    fallible("unlinkSync", "node_fs::unlink_sync", { kind: "tuple", elements: [] }, [stringCarrier]),
    fallible("symlinkSync", "node_fs::symlink_sync", { kind: "tuple", elements: [] }, [stringCarrier, stringCarrier]),
    fallible("copyFileSync", "node_fs::copy_file_sync", { kind: "tuple", elements: [] }, [stringCarrier, stringCarrier]),
    fallible("renameSync", "node_fs::rename_sync", { kind: "tuple", elements: [] }, [stringCarrier, stringCarrier]),
    fallible("realpathSync", "node_fs::realpath_sync", stringCarrier, [stringCarrier]),
    { exportId: statsId, memberId: `${statsId}.isFile`, operationKind: "method", target: { form: "receiver-method", name: "is_file" }, resultCarrier: boolCarrier },
    { exportId: statsId, memberId: `${statsId}.isDirectory`, operationKind: "method", target: { form: "receiver-method", name: "is_directory" }, resultCarrier: boolCarrier },
    { exportId: statsId, memberId: `${statsId}.isSymbolicLink`, operationKind: "method", target: { form: "receiver-method", name: "is_symbolic_link" }, resultCarrier: boolCarrier },
    { exportId: statsId, memberId: `${statsId}.size`, operationKind: "property", target: { form: "field", name: "size" }, resultCarrier: float64Carrier, resultConversion: rustUint64ToFloat64ValueConversion },
    { exportId: statsId, memberId: `${statsId}.mtimeMs`, operationKind: "property", target: { form: "receiver-method", name: "mtime_ms" }, resultCarrier: float64Carrier },
  ];
}

// --- node:fs/promises --------------------------------------------------------

function fsPromisesModule(): RustProviderModuleDefinition {
  const m = "node:fs/promises";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.fs-promises",
    imports: [{ moduleSpecifier: "node:fs", namedImports: [{ exportedName: "Stats" }] }],
    exports: [
      fnExport(m, "readFile", [{ name: "path", type: stringType }, { name: "encoding", type: stringType }], stringType),
      fnExport(m, "writeFile", [{ name: "path", type: stringType }, { name: "data", type: stringType }, { name: "encoding", type: stringType }], voidType),
      fnExport(m, "readdir", [{ name: "path", type: stringType }], stringArrayType),
      fnExport(m, "stat", [{ name: "path", type: stringType }], providerRef("node:fs", "Stats")),
      // Contract: recursive.
      fnExport(m, "mkdir", [{ name: "path", type: stringType }], voidType),
      // Contract: recursive and force.
      fnExport(m, "rm", [{ name: "path", type: stringType }], voidType),
      fnExport(m, "unlink", [{ name: "path", type: stringType }], voidType),
      fnExport(m, "copyFile", [{ name: "from", type: stringType }, { name: "to", type: stringType }], voidType),
      fnExport(m, "rename", [{ name: "from", type: stringType }, { name: "to", type: stringType }], voidType),
    ],
  };
}

function fsPromisesRows(): readonly RustProviderOperationDefinition[] {
  const row = (name: string, path: string, resultCarrier: RustTargetTypeRef, parameterCount: number, trailingArguments?: readonly RustProviderConstantArgument[]): RustProviderOperationDefinition => ({
    exportId: `node:fs/promises::${name}`,
    operationKind: "method",
    target: {
      form: "call",
      path,
      argModes: Array.from({ length: parameterCount }, () => "ref" as const),
      ...(trailingArguments === undefined ? {} : { trailingArguments }),
    },
    resultCarrier,
    parameterCarriers: Array.from({ length: parameterCount }, () => stringCarrier),
    ...providerNativeFallibility,
    isAsync: true,
  });
  const unit: RustTargetTypeRef = { kind: "tuple", elements: [] };
  return [
    row("readFile", "node_fs_promises::read_file_string_async", stringCarrier, 2),
    row("writeFile", "node_fs_promises::write_file_string_async", unit, 3),
    row("readdir", "node_fs_promises::readdir_async", stringArrayCarrier, 1),
    row("stat", "node_fs_promises::stat_async", statsCarrier, 1),
    row("mkdir", "node_fs_promises::mkdir_async", unit, 1, [trueArgument]),
    row("rm", "node_fs_promises::rm_async", unit, 1, [trueArgument, trueArgument]),
    row("unlink", "node_fs_promises::unlink_async", unit, 1),
    row("copyFile", "node_fs_promises::copy_file_async", unit, 2),
    row("rename", "node_fs_promises::rename_async", unit, 2),
  ];
}

// --- node:process ------------------------------------------------------------

// node:process exposes the Node module shape: cwd() as a function and
// platform/arch/argv/pid/ppid/env as value exports. env is an indexed
// object whose reads preserve absence as undefined (Option carrier).
function processModule(): RustProviderModuleDefinition {
  const m = "node:process";
  const envId = "node:process::ProcessEnv";
  const defaultId = "node:process.default";
  const valueExport = (name: string, type: ProviderTypeExpr, documentation?: string) => ({
    id: `${m}::${name}`,
    name,
    kind: "value" as const,
    type,
    ...(documentation === undefined ? {} : { documentation }),
  });
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.process",
    exports: [
      fnExport(m, "cwd", [], stringType),
      {
        id: envId,
        name: "ProcessEnv",
        kind: "class" as const,
        members: [{
          id: `${envId}.indexer`,
          name: "indexer",
          kind: "indexer" as const,
          signatures: [{
            id: `${envId}.indexer(name)`,
            parameters: [{ name: "name", type: stringType }],
            returnType: { kind: "union", types: [stringType, undefinedType] },
          }],
        }],
      },
      valueExport("env", providerRef(m, "ProcessEnv")),
      valueExport("platform", stringType),
      valueExport("arch", stringType),
      valueExport("argv", stringArrayType),
      valueExport("pid", numberType),
      valueExport("ppid", numberType),
      valueExport("execPath", stringType),
      valueExport("exitCode", { kind: "union", types: [numberType, nullType] }),
      fnExport(m, "exit", [{ name: "code", type: numberType }], voidType),
      {
        id: defaultId,
        name: "NodeProcessModule",
        exportKind: "default",
        kind: "class",
        members: [
          methodMember(defaultId, "cwd", [], stringType, { static: true }),
          propertyMember(defaultId, "env", providerRef(m, "ProcessEnv"), { static: true }),
          propertyMember(defaultId, "platform", stringType, { static: true }),
          propertyMember(defaultId, "arch", stringType, { static: true }),
          propertyMember(defaultId, "argv", stringArrayType, { static: true }),
          propertyMember(defaultId, "pid", numberType, { static: true }),
          propertyMember(defaultId, "ppid", numberType, { static: true }),
          propertyMember(defaultId, "execPath", stringType, { static: true }),
          propertyMember(defaultId, "exitCode", { kind: "union", types: [numberType, nullType] }, {
            readonly: false,
            static: true,
          }),
          methodMember(defaultId, "exit", [{ name: "code", type: numberType }], voidType, { static: true }),
        ],
      },
    ],
  };
}

function processRows(): readonly RustProviderOperationDefinition[] {
  const m = "node:process";
  const defaultId = "node:process.default";
  return [
    { exportId: `${m}::cwd`, operationKind: "method", target: { form: "call", path: "node_process::cwd" }, resultCarrier: stringCarrier, ...providerNativeFallibility },
    { exportId: `${m}::platform`, operationKind: "property", target: { form: "call", path: "node_process::platform" }, resultCarrier: stringCarrier },
    { exportId: `${m}::arch`, operationKind: "property", target: { form: "call", path: "node_process::arch" }, resultCarrier: stringCarrier },
    { exportId: `${m}::argv`, operationKind: "property", target: { form: "call", path: "node_process::argv" }, resultCarrier: stringArrayCarrier },
    { exportId: `${m}::pid`, operationKind: "property", target: { form: "call", path: "node_process::pid" }, resultCarrier: int32Carrier, resultConversion: rustUint32ToInt32ValueConversion },
    { exportId: `${m}::ppid`, operationKind: "property", target: { form: "call", path: "node_process::ppid" }, resultCarrier: int32Carrier, resultConversion: rustUint32ToInt32ValueConversion },
    { exportId: `${m}::env`, operationKind: "property", target: { form: "marker" }, resultCarrier: processEnvCarrier },
    { exportId: `${m}::execPath`, operationKind: "property", target: { form: "call", path: "node_process::exec_path" }, resultCarrier: stringCarrier, ...providerNativeFallibility },
    { exportId: `${m}::exitCode`, operationKind: "property", target: { form: "call", path: "node_process::exit_code" }, resultCarrier: rustOptionTargetType(int32Carrier) },
    { exportId: `${m}::exitCode`, operationKind: "property-set", target: { form: "call", path: "node_process::set_exit_code" }, resultCarrier: unitCarrier, parameterCarriers: [rustOptionTargetType(int32Carrier)] },
    { exportId: `${m}::ProcessEnv`, memberId: `${m}::ProcessEnv.indexer`, operationKind: "indexer", target: { form: "call", path: "node_process::env_get", argModes: ["ref"] }, resultCarrier: rustOptionTargetType(stringCarrier), parameterCarriers: [stringCarrier] },
    { exportId: `${m}::exit`, operationKind: "method", target: { form: "call", path: "std::process::exit" }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [int32Carrier] },
    { exportId: defaultId, memberId: `${defaultId}.cwd`, signatureId: `${defaultId}.cwd()`, operationKind: "method", target: { form: "call", path: "node_process::cwd" }, resultCarrier: stringCarrier, ...providerNativeFallibility },
    { exportId: defaultId, memberId: `${defaultId}.platform`, operationKind: "property", target: { form: "call", path: "node_process::platform" }, resultCarrier: stringCarrier },
    { exportId: defaultId, memberId: `${defaultId}.arch`, operationKind: "property", target: { form: "call", path: "node_process::arch" }, resultCarrier: stringCarrier },
    { exportId: defaultId, memberId: `${defaultId}.argv`, operationKind: "property", target: { form: "call", path: "node_process::argv" }, resultCarrier: stringArrayCarrier },
    { exportId: defaultId, memberId: `${defaultId}.pid`, operationKind: "property", target: { form: "call", path: "node_process::pid" }, resultCarrier: int32Carrier, resultConversion: rustUint32ToInt32ValueConversion },
    { exportId: defaultId, memberId: `${defaultId}.ppid`, operationKind: "property", target: { form: "call", path: "node_process::ppid" }, resultCarrier: int32Carrier, resultConversion: rustUint32ToInt32ValueConversion },
    { exportId: defaultId, memberId: `${defaultId}.env`, operationKind: "property", target: { form: "marker" }, resultCarrier: processEnvCarrier },
    { exportId: defaultId, memberId: `${defaultId}.execPath`, operationKind: "property", target: { form: "call", path: "node_process::exec_path" }, resultCarrier: stringCarrier, ...providerNativeFallibility },
    { exportId: defaultId, memberId: `${defaultId}.exitCode`, operationKind: "property", target: { form: "call", path: "node_process::exit_code" }, resultCarrier: rustOptionTargetType(int32Carrier) },
    { exportId: defaultId, memberId: `${defaultId}.exitCode`, operationKind: "property-set", target: { form: "call", path: "node_process::set_exit_code" }, resultCarrier: unitCarrier, parameterCarriers: [rustOptionTargetType(int32Carrier)] },
    { exportId: defaultId, memberId: `${defaultId}.exit`, signatureId: `${defaultId}.exit(code)`, operationKind: "method", target: { form: "call", path: "std::process::exit" }, resultCarrier: unitCarrier, parameterCarriers: [int32Carrier] },
  ];
}

// --- node:buffer -------------------------------------------------------------

function bufferModule(): RustProviderModuleDefinition {
  const m = "node:buffer";
  const bufferId = "node:buffer::Buffer";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.buffer",
    exports: [
      {
        id: bufferId,
        name: "Buffer",
        kind: "class" as const,
        members: [
          {
            id: `${bufferId}.from`,
            name: "from",
            kind: "method" as const,
            static: true,
            signatures: [
              {
                id: `${bufferId}.from(string)`,
                parameters: [{ name: "value", type: stringType }],
                returnType: providerRef(m, "Buffer"),
              },
              {
                id: `${bufferId}.from(string,encoding)`,
                parameters: [{ name: "value", type: stringType }, { name: "encoding", type: stringType }],
                returnType: providerRef(m, "Buffer"),
              },
              {
                id: `${bufferId}.from(numberArray)`,
                parameters: [{ name: "value", type: { kind: "array", elementType: numberType } }],
                returnType: providerRef(m, "Buffer"),
              },
            ],
          },
          methodMember(bufferId, "alloc", [{ name: "size", type: numberType }], providerRef(m, "Buffer"), { static: true }),
          methodMember(bufferId, "byteLength", [{ name: "value", type: stringType }, { name: "encoding", type: stringType }], numberType, { static: true }),
          methodMember(bufferId, "concat", [{ name: "list", type: { kind: "array", elementType: providerRef(m, "Buffer") } }], providerRef(m, "Buffer"), { static: true }),
          methodMember(bufferId, "toString", [{ name: "encoding", type: stringType }], stringType),
          methodMember(bufferId, "readUInt8", [{ name: "offset", type: numberType }], numberType),
          methodMember(bufferId, "writeUInt8", [{ name: "value", type: numberType }, { name: "offset", type: numberType }], voidType),
          methodMember(bufferId, "equals", [{ name: "other", type: providerRef(m, "Buffer") }], booleanType),
          methodMember(bufferId, "compare", [{ name: "other", type: providerRef(m, "Buffer") }], numberType),
          propertyMember(bufferId, "length", numberType),
        ],
      },
      fnExport(m, "isBuffer", [{ name: "value", type: providerRef(m, "Buffer") }], booleanType),
      fnExport(m, "btoa", [{ name: "value", type: stringType }], stringType),
      fnExport(m, "atob", [{ name: "value", type: stringType }], stringType),
      fnExport(m, "isEncoding", [{ name: "encoding", type: stringType }], booleanType),
    ],
  };
}

function bufferRows(): readonly RustProviderOperationDefinition[] {
  const bufferId = "node:buffer::Buffer";
  return [
    { exportId: bufferId, memberId: `${bufferId}.from`, signatureId: `${bufferId}.from(string)`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::from_string", argModes: ["ref"], trailingArguments: [noneArgument] }, resultCarrier: bufferCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: bufferId, memberId: `${bufferId}.from`, signatureId: `${bufferId}.from(string,encoding)`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::from_string_enc", argModes: ["ref", "ref"] }, resultCarrier: bufferCarrier, parameterCarriers: [stringCarrier, stringCarrier], ...providerNativeFallibility },
    { exportId: bufferId, memberId: `${bufferId}.from`, signatureId: `${bufferId}.from(numberArray)`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::from_number_array", argModes: ["ref"] }, resultCarrier: bufferCarrier, parameterCarriers: [rustJsArrayTargetType(float64Carrier)] },
    { exportId: bufferId, memberId: `${bufferId}.alloc`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::alloc", argConversions: [rustInt32ToUsizeValueConversion] }, resultCarrier: bufferCarrier, parameterCarriers: [int32Carrier] },
    { exportId: bufferId, memberId: `${bufferId}.byteLength`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::byte_length_enc", argModes: ["ref", "ref"] }, resultCarrier: int32Carrier, parameterCarriers: [stringCarrier, stringCarrier], ...providerNativeFallibility, resultConversion: rustUsizeToInt32ValueConversion },
    { exportId: bufferId, memberId: `${bufferId}.concat`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::concat", argModes: ["ref"] }, resultCarrier: bufferCarrier, parameterCarriers: [rustJsArrayTargetType(bufferCarrier)], ...providerNativeFallibility },
    { exportId: bufferId, memberId: `${bufferId}.toString`, operationKind: "method", target: { form: "receiver-method", name: "to_string_enc", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: bufferId, memberId: `${bufferId}.readUInt8`, operationKind: "method", target: { form: "receiver-method", name: "read_u8", argConversions: [rustInt32ToUsizeValueConversion] }, resultCarrier: int32Carrier, parameterCarriers: [int32Carrier], ...providerNativeFallibility, resultConversion: rustUint8ToInt32ValueConversion },
    { exportId: bufferId, memberId: `${bufferId}.writeUInt8`, operationKind: "method", target: { form: "receiver-method", name: "set", argOrder: [1, 0], argConversions: [rustInt32ToUsizeValueConversion, rustInt32ToUint8ValueConversion], mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [int32Carrier, int32Carrier], ...providerNativeFallibility },
    { exportId: bufferId, memberId: `${bufferId}.equals`, operationKind: "method", target: { form: "receiver-method", name: "equals", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [bufferCarrier] },
    { exportId: bufferId, memberId: `${bufferId}.compare`, operationKind: "method", target: { form: "receiver-method", name: "compare", argModes: ["ref"] }, resultCarrier: int32Carrier, parameterCarriers: [bufferCarrier] },
    { exportId: bufferId, memberId: `${bufferId}.length`, operationKind: "property", target: { form: "receiver-method", name: "len" }, resultCarrier: int32Carrier, resultConversion: rustUsizeToInt32ValueConversion },
    { exportId: "node:buffer::btoa", operationKind: "method", target: { form: "call", path: "node_buffer::btoa", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: "node:buffer::atob", operationKind: "method", target: { form: "call", path: "node_buffer::atob", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: "node:buffer::isEncoding", operationKind: "method", target: { form: "call", path: "node_buffer::is_encoding", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [stringCarrier] },
    { exportId: "node:buffer::isBuffer", operationKind: "method", target: { form: "call", path: "node_buffer::is_buffer", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [bufferCarrier] },
  ];
}

// --- node:url ----------------------------------------------------------------

function urlModule(): RustProviderModuleDefinition {
  const m = "node:url";
  const urlId = "node:url::URL";
  const paramsId = "node:url::URLSearchParams";
  const urlObjectId = "node:url::UrlObject";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.url",
    exports: [
      {
        id: urlId,
        name: "URL",
        kind: "class" as const,
        members: [
          constructorMember(urlId, [{ name: "input", type: stringType }]),
          propertyMember(urlId, "href", stringType),
          propertyMember(urlId, "protocol", stringType),
          propertyMember(urlId, "host", stringType),
          propertyMember(urlId, "hostname", stringType),
          propertyMember(urlId, "port", stringType),
          propertyMember(urlId, "pathname", stringType),
          propertyMember(urlId, "search", stringType),
          propertyMember(urlId, "hash", stringType),
          propertyMember(urlId, "origin", stringType),
        ],
      },
      {
        id: paramsId,
        name: "URLSearchParams",
        kind: "class" as const,
        members: [
          constructorMember(paramsId, [{ name: "init", type: stringType }]),
          methodMember(paramsId, "get", [{ name: "name", type: stringType }], { kind: "union", types: [stringType, nullType] }),
          methodMember(paramsId, "set", [{ name: "name", type: stringType }, { name: "value", type: stringType }], voidType),
          methodMember(paramsId, "append", [{ name: "name", type: stringType }, { name: "value", type: stringType }], voidType),
          methodMember(paramsId, "has", [{ name: "name", type: stringType }], booleanType),
          methodMember(paramsId, "toString", [], stringType),
        ],
      },
      {
        id: urlObjectId,
        name: "UrlObject",
        kind: "class" as const,
        members: [
          propertyMember(urlObjectId, "href", stringType),
          propertyMember(urlObjectId, "protocol", stringType),
          propertyMember(urlObjectId, "host", stringType),
          propertyMember(urlObjectId, "hostname", stringType),
          propertyMember(urlObjectId, "port", stringType),
          propertyMember(urlObjectId, "pathname", stringType),
          propertyMember(urlObjectId, "search", stringType),
          propertyMember(urlObjectId, "hash", stringType),
        ],
      },
      fnExport(m, "pathToFileURL", [{ name: "path", type: stringType }], providerRef(m, "URL")),
      fnExport(m, "fileURLToPath", [{ name: "url", type: providerRef(m, "URL") }], stringType),
      fnExport(m, "canParse", [{ name: "input", type: stringType }], booleanType),
      fnExport(m, "parse", [{ name: "input", type: stringType }], providerRef(m, "UrlObject")),
      fnExport(m, "format", [{ name: "url", type: providerRef(m, "UrlObject") }], stringType),
    ],
  };
}

function urlRows(): readonly RustProviderOperationDefinition[] {
  const urlId = "node:url::URL";
  const paramsId = "node:url::URLSearchParams";
  const urlObjectId = "node:url::UrlObject";
  const urlProperty = (name: string): RustProviderOperationDefinition => ({
    exportId: urlId,
    memberId: `${urlId}.${name}`,
    operationKind: "property",
    target: { form: "receiver-method", name },
    resultCarrier: stringCarrier,
  });
  const urlObjectProperty = (name: string): RustProviderOperationDefinition => ({
    exportId: urlObjectId,
    memberId: `${urlObjectId}.${name}`,
    operationKind: "property",
    target: { form: "receiver-method", name },
    resultCarrier: stringCarrier,
  });
  return [
    { exportId: urlId, memberId: `${urlId}.constructor`, operationKind: "constructor", target: { form: "call", path: "node_url::Url::parse", argModes: ["ref"], trailingArguments: [noneArgument] }, resultCarrier: urlCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    ...["href", "protocol", "host", "hostname", "port", "pathname", "search", "hash", "origin"].map(urlProperty),
    { exportId: paramsId, memberId: `${paramsId}.constructor`, operationKind: "constructor", target: { form: "call", path: "node_url::UrlSearchParams::new_from", argModes: ["ref"] }, resultCarrier: searchParamsCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: paramsId, memberId: `${paramsId}.get`, operationKind: "method", target: { form: "receiver-method", name: "get", argModes: ["ref"] }, resultCarrier: rustOptionTargetType(stringCarrier), parameterCarriers: [stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.set`, operationKind: "method", target: { form: "receiver-method", name: "set", argModes: ["ref", "ref"], mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [stringCarrier, stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.append`, operationKind: "method", target: { form: "receiver-method", name: "append", argModes: ["ref", "ref"], mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [stringCarrier, stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.has`, operationKind: "method", target: { form: "receiver-method", name: "has", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.toString`, operationKind: "method", target: { form: "receiver-method", name: "to_string" }, resultCarrier: stringCarrier },
    { exportId: "node:url::pathToFileURL", operationKind: "method", target: { form: "call", path: "node_url::path_to_file_url", argModes: ["ref"] }, resultCarrier: urlCarrier, parameterCarriers: [stringCarrier] },
    { exportId: "node:url::canParse", operationKind: "method", target: { form: "call", path: "node_url::can_parse", argModes: ["ref"], trailingArguments: [noneArgument] }, resultCarrier: boolCarrier, parameterCarriers: [stringCarrier] },
    { exportId: "node:url::fileURLToPath", operationKind: "method", target: { form: "call", path: "node_url::file_url_to_path", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [urlCarrier], ...providerNativeFallibility },
    { exportId: "node:url::parse", operationKind: "method", target: { form: "call", path: "node_url::parse_legacy", argModes: ["ref"] }, resultCarrier: urlObjectCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    ...["href", "protocol", "host", "hostname", "port", "pathname", "search", "hash"].map(urlObjectProperty),
    { exportId: "node:url::format", operationKind: "method", target: { form: "call", path: "node_url::format_legacy", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [urlObjectCarrier] },
  ];
}

// --- node:crypto -------------------------------------------------------------

function cryptoModule(): RustProviderModuleDefinition {
  const m = "node:crypto";
  const hashId = "node:crypto::Hash";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.crypto",
    imports: [{ moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] }],
    exports: [
      fnExport(m, "randomUUID", [], stringType),
      fnExport(m, "createHash", [{ name: "algorithm", type: stringType }], providerRef(m, "Hash")),
      {
        id: hashId,
        name: "Hash",
        kind: "class" as const,
        members: [
          {
            id: `${hashId}.update`,
            name: "update",
            kind: "method" as const,
            signatures: [
              {
                id: `${hashId}.update(string)`,
                parameters: [{ name: "value", type: stringType }],
                returnType: providerRef(m, "Hash"),
              },
              {
                id: `${hashId}.update(buffer)`,
                parameters: [{ name: "value", type: providerRef("node:buffer", "Buffer") }],
                returnType: providerRef(m, "Hash"),
              },
            ],
          },
          methodMember(hashId, "digest", [{ name: "encoding", type: stringType }], stringType),
        ],
      },
      fnExport(m, "createHmac", [{ name: "algorithm", type: stringType }, { name: "key", type: stringType }], providerRef(m, "Hmac")),
      {
        id: "node:crypto::Hmac",
        name: "Hmac",
        kind: "class" as const,
        members: [
          methodMember("node:crypto::Hmac", "update", [{ name: "value", type: stringType }], voidType),
          methodMember("node:crypto::Hmac", "digest", [{ name: "encoding", type: stringType }], stringType),
        ],
      },
      fnExport(m, "randomBytes", [{ name: "size", type: numberType }], providerRef("node:buffer", "Buffer")),
    ],
  };
}

function cryptoRows(): readonly RustProviderOperationDefinition[] {
  const hashId = "node:crypto::Hash";
  return [
    { exportId: "node:crypto::randomUUID", operationKind: "method", target: { form: "call", path: "node_crypto::random_uuid" }, resultCarrier: stringCarrier, ...providerNativeFallibility },
    { exportId: "node:crypto::createHash", operationKind: "method", target: { form: "call", path: "node_crypto::create_hash", argModes: ["ref"] }, resultCarrier: hashCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    {
      exportId: hashId,
      memberId: `${hashId}.update`,
      signatureId: `${hashId}.update(string)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "update_str_owned", argModes: ["ref"], mutatesReceiver: true },
      resultCarrier: hashCarrier,
      parameterCarriers: [stringCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: hashId,
      memberId: `${hashId}.update`,
      signatureId: `${hashId}.update(buffer)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "update_buffer_owned", argModes: ["ref"], mutatesReceiver: true },
      resultCarrier: hashCarrier,
      parameterCarriers: [bufferCarrier],
      ...providerNativeFallibility,
    },
    { exportId: hashId, memberId: `${hashId}.digest`, operationKind: "method", target: { form: "receiver-method", name: "digest_string", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: "node:crypto::createHmac", operationKind: "method", target: { form: "call", path: "node_crypto::create_hmac_str", argModes: ["ref", "ref"] }, resultCarrier: hmacCarrier, parameterCarriers: [stringCarrier, stringCarrier], ...providerNativeFallibility },
    { exportId: "node:crypto::Hmac", memberId: "node:crypto::Hmac.update", operationKind: "method", target: { form: "receiver-method", name: "update_str", argModes: ["ref"], mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: "node:crypto::Hmac", memberId: "node:crypto::Hmac.digest", operationKind: "method", target: { form: "receiver-method", name: "digest_string", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: "node:crypto::randomBytes", operationKind: "method", target: { form: "call", path: "node_crypto::random_bytes", argConversions: [rustInt32ToUsizeValueConversion] }, resultCarrier: bufferCarrier, parameterCarriers: [int32Carrier], ...providerNativeFallibility },
  ];
}

// --- node:util ---------------------------------------------------------------

function utilModule(): RustProviderModuleDefinition {
  const m = "node:util";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.util",
    exports: [
      fnExport(m, "stripVTControlCharacters", [{ name: "value", type: stringType }], stringType),
      fnExport(m, "toUSVString", [{ name: "value", type: stringType }], stringType),
      fnExport(m, "styleText", [{ name: "style", type: stringType }, { name: "text", type: stringType }], stringType),
      fnExport(m, "getSystemErrorName", [{ name: "code", type: numberType }], stringType),
      fnExport(m, "getSystemErrorMessage", [{ name: "code", type: numberType }], stringType),
      fnExport(m, "inspect", [{ name: "value", type: { kind: "any" } }], stringType),
      fnExport(m, "format", [{ name: "format", type: stringType }, { name: "values", type: { kind: "array", elementType: { kind: "any" } }, rest: true }], stringType),
    ],
  };
}

function utilRows(): readonly RustProviderOperationDefinition[] {
  const m = "node:util";
  return [
    { exportId: `${m}::stripVTControlCharacters`, operationKind: "method", target: { form: "call", path: "node_util::strip_vt_control_characters", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier] },
    { exportId: `${m}::toUSVString`, operationKind: "method", target: { form: "call", path: "node_util::to_usv_string", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier] },
    { exportId: `${m}::styleText`, operationKind: "method", target: { form: "call", path: "node_util::style_text", argModes: ["ref", "ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier, stringCarrier] },
    { exportId: `${m}::getSystemErrorName`, operationKind: "method", target: { form: "call", path: "node_util::get_system_error_name", chain: [toStringStep] }, resultCarrier: stringCarrier, parameterCarriers: [int32Carrier] },
    { exportId: `${m}::inspect`, operationKind: "method", target: { form: "call", path: "node_util::inspect", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [jsValueCarrier] },
    { exportId: `${m}::getSystemErrorMessage`, operationKind: "method", target: { form: "call", path: "node_util::get_system_error_message", chain: [toStringStep] }, resultCarrier: stringCarrier, parameterCarriers: [int32Carrier] },
    { exportId: `${m}::format`, operationKind: "method", target: { form: "call-value-slice", path: "node_util::format", leadingArguments: [{ carrier: stringCarrier, mode: "ref" }], elementCarrier: jsValueCarrier }, resultCarrier: stringCarrier },
  ];
}

export function createRustNodejsProviderPackage(): RustProviderPackageImplementation {
  return createRustProviderPackage({
    id: "@tsonic/rust-nodejs",
    displayName: "Node.js for Rust",
    version: "0.0.1",
    requiredSurfaces: ["js"],
    modules: [
      assertModule(),
      pathModule(),
      osModule(),
      fsModule(),
      fsPromisesModule(),
      processModule(),
      bufferModule(),
      urlModule(),
      cryptoModule(),
      utilModule(),
      httpModule(),
      timersModule(),
    ],
    types: [
      { exportId: "node:fs::Stats", targetCarrier: statsCarrier },
      { exportId: "node:process::ProcessEnv", targetCarrier: processEnvCarrier },
      { exportId: "node:buffer::Buffer", targetCarrier: bufferCarrier },
      { exportId: "node:url::URL", targetCarrier: urlCarrier },
      { exportId: "node:url::UrlObject", targetCarrier: urlObjectCarrier },
      { exportId: "node:url::URLSearchParams", targetCarrier: searchParamsCarrier },
      { exportId: "node:crypto::Hash", targetCarrier: hashCarrier },
      { exportId: "node:crypto::Hmac", targetCarrier: hmacCarrier },
      { exportId: "node:http::IncomingMessage", targetCarrier: httpIncomingMessageCarrier },
      { exportId: "node:http::ServerResponse", targetCarrier: httpServerResponseCarrier },
      { exportId: "node:http::Server", targetCarrier: httpServerCarrier },
      { exportId: "node:timers::Timeout", targetCarrier: timeoutCarrier },
    ],
    operations: [
      ...assertRows(),
      ...pathRows(),
      ...osRows(),
      ...fsRows(),
      ...fsPromisesRows(),
      ...processRows(),
      ...bufferRows(),
      ...urlRows(),
      ...cryptoRows(),
      ...utilRows(),
      ...httpRows(),
      ...timersRows(),
    ],
    aliasImports: [
      { alias: "node_assert", path: "tsonic_rust_node::assert" },
      { alias: "node_path", path: "tsonic_rust_node::path" },
      { alias: "node_os", path: "tsonic_rust_node::os" },
      { alias: "node_fs", path: "tsonic_rust_node::fs" },
      { alias: "node_fs_promises", path: "tsonic_rust_node::fs_promises" },
      { alias: "node_process", path: "tsonic_rust_node::process" },
      { alias: "node_buffer", path: "tsonic_rust_node::buffer" },
      { alias: "node_url", path: "tsonic_rust_node::url" },
      { alias: "node_crypto", path: "tsonic_rust_node::crypto" },
      { alias: "node_util", path: "tsonic_rust_node::util" },
      { alias: "node_http", path: "tsonic_rust_node::http" },
      { alias: "node_timers", path: "tsonic_rust_node::timers" },
    ],
    carrierPaths: {
      "rust.node.Stats": "tsonic_rust_node::fs::Stats",
      "rust.node.Buffer": "tsonic_rust_node::buffer::Buffer",
      "rust.node.Url": "tsonic_rust_node::url::Url",
      "rust.node.UrlObject": "tsonic_rust_node::url::LegacyUrlObject",
      "rust.node.UrlSearchParams": "tsonic_rust_node::url::UrlSearchParams",
      "rust.node.Hash": "tsonic_rust_node::crypto::Hash",
      "rust.node.Hmac": "tsonic_rust_node::crypto::Hmac",
      "rust.node.ProcessEnv": "tsonic_rust_node::process::ProcessEnv",
      "rust.node.HttpIncomingMessage": "tsonic_rust_node::http::IncomingMessage",
      "rust.node.HttpServerResponse": "tsonic_rust_node::http::ServerResponseHandle",
      "rust.node.HttpServer": "tsonic_rust_node::http::ServerHandle",
      "rust.node.Timeout": "tsonic_rust_node::timers::Timeout",
    },
    carrierTraits: {
      "rust.node.Stats": { clone: "always", copy: "never" },
      "rust.node.Buffer": { clone: "always", copy: "never" },
      "rust.node.Url": { clone: "always", copy: "never" },
      "rust.node.UrlObject": { clone: "always", copy: "never" },
      "rust.node.UrlSearchParams": { clone: "always", copy: "never" },
      "rust.node.Hash": { clone: "always", copy: "never" },
      "rust.node.Hmac": { clone: "always", copy: "never" },
      "rust.node.HttpIncomingMessage": { clone: "always", copy: "never" },
      "rust.node.HttpServerResponse": { clone: "always", copy: "never" },
      "rust.node.HttpServer": { clone: "always", copy: "never" },
      "rust.node.Timeout": { clone: "always", copy: "never" },
    },
    binaryEpilogues: [{
      id: "node-event-loop",
      path: "tsonic_rust_node::run_event_loop",
      requiredCrate: "tsonic_rust_node",
      isFallible: true,
      errorBoundary: "source-program",
    }, {
      id: "node-process-exit-code",
      path: "tsonic_rust_node::process::apply_exit_code",
      requiredCrate: "tsonic_rust_node",
    }],
    crates: [{
      crateName: "tsonic_rust_node",
      cargoPath: resolve(packageRoot, "rust/crates/tsonic_rust_node"),
      registryPatch: "crates-io",
    }],
  });
}
