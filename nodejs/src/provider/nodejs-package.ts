import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  createRustProviderPackage,
  rustInt32ToUint8ValueConversion,
  rustInt32ToUsizeValueConversion,
  rustOptionTargetType,
  rustSourcePrimitiveTargetType,
  rustStringTargetType,
  rustUint32ToInt32ValueConversion,
  rustUint64ToFloat64ValueConversion,
  rustUint8ToInt32ValueConversion,
  rustUsizeToInt32ValueConversion,
  rustVecTargetType,
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
const stringVecCarrier = rustVecTargetType(stringCarrier);
const statsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Stats" };
const bufferCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Buffer" };
const urlCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Url" };
const urlObjectCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.UrlObject" };
const searchParamsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.UrlSearchParams" };
const hashCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Hash" };
const hmacCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Hmac" };
const trueArgument = { kind: "boolean", value: true } as const;
const noneArgument = { kind: "none" } as const;
const toStringStep = { kind: "method", name: "to_string" } as const;

const stringType = { kind: "string" } as const;
const numberType = { kind: "number" } as const;
const booleanType = { kind: "boolean" } as const;
const voidType = { kind: "void" } as const;
const stringArrayType = { kind: "array", elementType: stringType } as const;

const nullType = { kind: "literal", value: null } as const;

type ProviderTypeExpr =
  | typeof stringType
  | typeof numberType
  | typeof booleanType
  | typeof voidType
  | typeof stringArrayType
  | { readonly kind: "provider-ref"; readonly moduleSpecifier: string; readonly exportName: string }
  | { readonly kind: "array"; readonly elementType: ProviderTypeExpr }
  | { readonly kind: "union"; readonly types: readonly ProviderTypeExpr[] }
  | typeof nullType
  | { readonly kind: "any" };

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

function propertyMember(classId: string, name: string, type: ProviderTypeExpr) {
  return {
    id: `${classId}.${name}`,
    name,
    kind: "property" as const,
    readonly: true,
    type,
  };
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
      isFallible: true,
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
      isFallible: true,
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
    { exportId: "node:path::resolve", operationKind: "method", target: { form: "call-str-slice", path: "node_path::resolve" }, resultCarrier: stringCarrier, isFallible: true },
    simple("normalize", "node_path::normalize"),
    simple("dirname", "node_path::dirname"),
    simple("extname", "node_path::extname"),
    simple("isAbsolute", "node_path::is_absolute", boolCarrier),
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
  const call = (name: string, path: string, extra?: Partial<RustProviderOperationDefinition>): RustProviderOperationDefinition => ({
    exportId: `node:os::${name}`,
    operationKind: "method",
    target: { form: "call", path },
    resultCarrier: stringCarrier,
    ...extra,
  });
  return [
    call("platform", "node_os::platform"),
    call("arch", "node_os::arch"),
    { exportId: "node:os::eol", operationKind: "method", target: { form: "call", path: "node_os::eol", chain: [toStringStep] }, resultCarrier: stringCarrier },
    call("hostname", "node_os::hostname"),
    call("tmpdir", "node_os::tmpdir", { isFallible: true }),
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
    exports: [
      fnExport(m, "existsSync", [{ name: "path", type: stringType }], booleanType),
      fnExport(m, "readFileSync", [{ name: "path", type: stringType }, { name: "encoding", type: stringType }], stringType),
      fnExport(m, "writeFileSync", [{ name: "path", type: stringType }, { name: "data", type: stringType }, { name: "encoding", type: stringType }], voidType),
      fnExport(m, "readdirSync", [{ name: "path", type: stringType }], stringArrayType),
      fnExport(m, "statSync", [{ name: "path", type: stringType }], providerRef(m, "Stats")),
      // Contract: recursive.
      fnExport(m, "mkdirSync", [{ name: "path", type: stringType }], voidType),
      // Contract: recursive and force.
      fnExport(m, "rmSync", [{ name: "path", type: stringType }], voidType),
      fnExport(m, "unlinkSync", [{ name: "path", type: stringType }], voidType),
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
          propertyMember(statsId, "size", numberType),
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
    isFallible: true,
  });
  return [
    { exportId: "node:fs::existsSync", operationKind: "method", target: { form: "call", path: "node_fs::exists_sync", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [stringCarrier] },
    fallible("readFileSync", "node_fs::read_file_sync_string", stringCarrier, [stringCarrier, stringCarrier]),
    fallible("writeFileSync", "node_fs::write_file_sync_string", { kind: "tuple", elements: [] }, [stringCarrier, stringCarrier, stringCarrier]),
    fallible("readdirSync", "node_fs::readdir_sync", stringVecCarrier, [stringCarrier]),
    fallible("statSync", "node_fs::stat_sync", statsCarrier, [stringCarrier]),
    fallible("mkdirSync", "node_fs::mkdir_sync", { kind: "tuple", elements: [] }, [stringCarrier], [trueArgument]),
    fallible("rmSync", "node_fs::rm_sync", { kind: "tuple", elements: [] }, [stringCarrier], [trueArgument, trueArgument]),
    fallible("unlinkSync", "node_fs::unlink_sync", { kind: "tuple", elements: [] }, [stringCarrier]),
    fallible("copyFileSync", "node_fs::copy_file_sync", { kind: "tuple", elements: [] }, [stringCarrier, stringCarrier]),
    fallible("renameSync", "node_fs::rename_sync", { kind: "tuple", elements: [] }, [stringCarrier, stringCarrier]),
    fallible("realpathSync", "node_fs::realpath_sync", stringCarrier, [stringCarrier]),
    { exportId: statsId, memberId: `${statsId}.isFile`, operationKind: "method", target: { form: "receiver-method", name: "is_file" }, resultCarrier: boolCarrier },
    { exportId: statsId, memberId: `${statsId}.isDirectory`, operationKind: "method", target: { form: "receiver-method", name: "is_directory" }, resultCarrier: boolCarrier },
    { exportId: statsId, memberId: `${statsId}.size`, operationKind: "property", target: { form: "field", name: "size" }, resultCarrier: float64Carrier, resultConversion: rustUint64ToFloat64ValueConversion },
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
    isFallible: true,
    isAsync: true,
  });
  const unit: RustTargetTypeRef = { kind: "tuple", elements: [] };
  return [
    row("readFile", "node_fs_promises::read_file_string_async", stringCarrier, 2),
    row("writeFile", "node_fs_promises::write_file_string_async", unit, 3),
    row("readdir", "node_fs_promises::readdir_async", stringVecCarrier, 1),
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
// object whose reads preserve absence as null (Option carrier).
function processModule(): RustProviderModuleDefinition {
  const m = "node:process";
  const envId = "node:process::ProcessEnv";
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
            returnType: { kind: "union", types: [stringType, nullType] },
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
      fnExport(m, "exit", [{ name: "code", type: numberType }], voidType),
    ],
  };
}

function processRows(): readonly RustProviderOperationDefinition[] {
  const m = "node:process";
  const envCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ProcessEnv" };
  return [
    { exportId: `${m}::cwd`, operationKind: "method", target: { form: "call", path: "node_process::cwd" }, resultCarrier: stringCarrier, isFallible: true },
    { exportId: `${m}::platform`, operationKind: "property", target: { form: "call", path: "node_process::platform" }, resultCarrier: stringCarrier },
    { exportId: `${m}::arch`, operationKind: "property", target: { form: "call", path: "node_process::arch" }, resultCarrier: stringCarrier },
    { exportId: `${m}::argv`, operationKind: "property", target: { form: "call", path: "node_process::argv" }, resultCarrier: stringVecCarrier },
    { exportId: `${m}::pid`, operationKind: "property", target: { form: "call", path: "node_process::pid" }, resultCarrier: int32Carrier, resultConversion: rustUint32ToInt32ValueConversion },
    { exportId: `${m}::ppid`, operationKind: "property", target: { form: "call", path: "node_process::ppid" }, resultCarrier: int32Carrier, resultConversion: rustUint32ToInt32ValueConversion },
    { exportId: `${m}::env`, operationKind: "property", target: { form: "marker" }, resultCarrier: envCarrier },
    { exportId: `${m}::execPath`, operationKind: "property", target: { form: "call", path: "node_process::exec_path" }, resultCarrier: stringCarrier, isFallible: true },
    { exportId: `${m}::ProcessEnv`, memberId: `${m}::ProcessEnv.indexer`, operationKind: "indexer", target: { form: "call", path: "node_process::env_get", argModes: ["ref"] }, resultCarrier: rustOptionTargetType(stringCarrier), parameterCarriers: [stringCarrier] },
    { exportId: `${m}::exit`, operationKind: "method", target: { form: "call", path: "std::process::exit" }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [int32Carrier] },
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
          methodMember(bufferId, "from", [{ name: "value", type: stringType }, { name: "encoding", type: stringType }], providerRef(m, "Buffer"), { static: true }),
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
    { exportId: bufferId, memberId: `${bufferId}.from`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::from_string_enc", argModes: ["ref", "ref"] }, resultCarrier: bufferCarrier, parameterCarriers: [stringCarrier, stringCarrier], isFallible: true },
    { exportId: bufferId, memberId: `${bufferId}.alloc`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::alloc", argConversions: [rustInt32ToUsizeValueConversion] }, resultCarrier: bufferCarrier, parameterCarriers: [int32Carrier] },
    { exportId: bufferId, memberId: `${bufferId}.byteLength`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::byte_length_enc", argModes: ["ref", "ref"] }, resultCarrier: int32Carrier, parameterCarriers: [stringCarrier, stringCarrier], isFallible: true, resultConversion: rustUsizeToInt32ValueConversion },
    { exportId: bufferId, memberId: `${bufferId}.concat`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::concat", argModes: ["ref"] }, resultCarrier: bufferCarrier, parameterCarriers: [rustVecTargetType(bufferCarrier)] },
    { exportId: bufferId, memberId: `${bufferId}.toString`, operationKind: "method", target: { form: "receiver-method", name: "to_string_enc", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], isFallible: true },
    { exportId: bufferId, memberId: `${bufferId}.readUInt8`, operationKind: "method", target: { form: "receiver-method", name: "read_u8", argConversions: [rustInt32ToUsizeValueConversion] }, resultCarrier: int32Carrier, parameterCarriers: [int32Carrier], isFallible: true, resultConversion: rustUint8ToInt32ValueConversion },
    { exportId: bufferId, memberId: `${bufferId}.writeUInt8`, operationKind: "method", target: { form: "receiver-method", name: "set", argOrder: [1, 0], argConversions: [rustInt32ToUsizeValueConversion, rustInt32ToUint8ValueConversion], mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [int32Carrier, int32Carrier], isFallible: true },
    { exportId: bufferId, memberId: `${bufferId}.equals`, operationKind: "method", target: { form: "receiver-method", name: "equals", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [bufferCarrier] },
    { exportId: bufferId, memberId: `${bufferId}.compare`, operationKind: "method", target: { form: "receiver-method", name: "compare", argModes: ["ref"] }, resultCarrier: int32Carrier, parameterCarriers: [bufferCarrier] },
    { exportId: bufferId, memberId: `${bufferId}.length`, operationKind: "property", target: { form: "receiver-method", name: "len" }, resultCarrier: int32Carrier, resultConversion: rustUsizeToInt32ValueConversion },
    { exportId: "node:buffer::btoa", operationKind: "method", target: { form: "call", path: "node_buffer::btoa", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], isFallible: true },
    { exportId: "node:buffer::atob", operationKind: "method", target: { form: "call", path: "node_buffer::atob", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], isFallible: true },
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
    { exportId: urlId, memberId: `${urlId}.constructor`, operationKind: "constructor", target: { form: "call", path: "node_url::Url::parse", argModes: ["ref"], trailingArguments: [noneArgument] }, resultCarrier: urlCarrier, parameterCarriers: [stringCarrier], isFallible: true },
    ...["href", "protocol", "host", "hostname", "port", "pathname", "search", "hash", "origin"].map(urlProperty),
    { exportId: paramsId, memberId: `${paramsId}.constructor`, operationKind: "constructor", target: { form: "call", path: "node_url::UrlSearchParams::new_from", argModes: ["ref"] }, resultCarrier: searchParamsCarrier, parameterCarriers: [stringCarrier], isFallible: true },
    { exportId: paramsId, memberId: `${paramsId}.get`, operationKind: "method", target: { form: "receiver-method", name: "get", argModes: ["ref"] }, resultCarrier: rustOptionTargetType(stringCarrier), parameterCarriers: [stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.set`, operationKind: "method", target: { form: "receiver-method", name: "set", argModes: ["ref", "ref"], mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [stringCarrier, stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.append`, operationKind: "method", target: { form: "receiver-method", name: "append", argModes: ["ref", "ref"], mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [stringCarrier, stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.has`, operationKind: "method", target: { form: "receiver-method", name: "has", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.toString`, operationKind: "method", target: { form: "receiver-method", name: "to_string" }, resultCarrier: stringCarrier },
    { exportId: "node:url::pathToFileURL", operationKind: "method", target: { form: "call", path: "node_url::path_to_file_url", argModes: ["ref"] }, resultCarrier: urlCarrier, parameterCarriers: [stringCarrier] },
    { exportId: "node:url::canParse", operationKind: "method", target: { form: "call", path: "node_url::can_parse", argModes: ["ref"], trailingArguments: [noneArgument] }, resultCarrier: boolCarrier, parameterCarriers: [stringCarrier] },
    { exportId: "node:url::fileURLToPath", operationKind: "method", target: { form: "call", path: "node_url::file_url_to_path", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [urlCarrier], isFallible: true },
    { exportId: "node:url::parse", operationKind: "method", target: { form: "call", path: "node_url::parse_legacy", argModes: ["ref"] }, resultCarrier: urlObjectCarrier, parameterCarriers: [stringCarrier], isFallible: true },
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
          methodMember(hashId, "update", [{ name: "value", type: stringType }], voidType),
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
    { exportId: "node:crypto::randomUUID", operationKind: "method", target: { form: "call", path: "node_crypto::random_uuid" }, resultCarrier: stringCarrier, isFallible: true },
    { exportId: "node:crypto::createHash", operationKind: "method", target: { form: "call", path: "node_crypto::create_hash", argModes: ["ref"] }, resultCarrier: hashCarrier, parameterCarriers: [stringCarrier], isFallible: true },
    { exportId: hashId, memberId: `${hashId}.update`, operationKind: "method", target: { form: "receiver-method", name: "update_str", argModes: ["ref"], mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [stringCarrier], isFallible: true },
    { exportId: hashId, memberId: `${hashId}.digest`, operationKind: "method", target: { form: "receiver-method", name: "digest_string", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], isFallible: true },
    { exportId: "node:crypto::createHmac", operationKind: "method", target: { form: "call", path: "node_crypto::create_hmac_str", argModes: ["ref", "ref"] }, resultCarrier: hmacCarrier, parameterCarriers: [stringCarrier, stringCarrier], isFallible: true },
    { exportId: "node:crypto::Hmac", memberId: "node:crypto::Hmac.update", operationKind: "method", target: { form: "receiver-method", name: "update_str", argModes: ["ref"], mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [stringCarrier], isFallible: true },
    { exportId: "node:crypto::Hmac", memberId: "node:crypto::Hmac.digest", operationKind: "method", target: { form: "receiver-method", name: "digest_string", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], isFallible: true },
    { exportId: "node:crypto::randomBytes", operationKind: "method", target: { form: "call", path: "node_crypto::random_bytes", argConversions: [rustInt32ToUsizeValueConversion] }, resultCarrier: bufferCarrier, parameterCarriers: [int32Carrier], isFallible: true },
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
    ],
    types: [
      { exportId: "node:fs::Stats", targetTypeId: "rust.node.Stats" },
      { exportId: "node:process::ProcessEnv", targetTypeId: "rust.node.ProcessEnv" },
      { exportId: "node:buffer::Buffer", targetTypeId: "rust.node.Buffer" },
      { exportId: "node:url::URL", targetTypeId: "rust.node.Url" },
      { exportId: "node:url::UrlObject", targetTypeId: "rust.node.UrlObject" },
      { exportId: "node:url::URLSearchParams", targetTypeId: "rust.node.UrlSearchParams" },
      { exportId: "node:crypto::Hash", targetTypeId: "rust.node.Hash" },
      { exportId: "node:crypto::Hmac", targetTypeId: "rust.node.Hmac" },
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
    ],
    carrierPaths: {
      "rust.node.Stats": "node_fs::Stats",
      "rust.node.Buffer": "node_buffer::Buffer",
      "rust.node.Url": "node_url::Url",
      "rust.node.UrlObject": "node_url::LegacyUrlObject",
      "rust.node.UrlSearchParams": "node_url::UrlSearchParams",
      "rust.node.Hash": "node_crypto::Hash",
      "rust.node.Hmac": "node_crypto::Hmac",
      "rust.node.ProcessEnv": "node_process::ProcessEnv",
    },
    crates: [{
      crateName: "tsonic_rust_node",
      cargoPath: resolve(packageRoot, "rust/crates/tsonic_rust_node"),
      registryPatch: "crates-io",
    }],
  });
}
