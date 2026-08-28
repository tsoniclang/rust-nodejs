import {
  boolCarrier,
  booleanType,
  bufferCarrier,
  float64Carrier,
  fileWatchCallbackCarrier,
  fileStatWatchCallbackCarrier,
  fnExport,
  makeDirectoryOptionsCarrier,
  methodMember,
  numberType,
  propertyMember,
  providerNativeFallibility,
  providerRef,
  readStreamCarrier,
  readStreamOptionsCarrier,
  fsWatcherCarrier,
  httpServerResponseCarrier,
  rmOptionsCarrier,
  rustOptionTargetType,
  rustUint64ToFloat64ValueConversion,
  statsCarrier,
  stringArrayCarrier,
  stringArrayType,
  stringCarrier,
  stringType,
  unitCarrier,
  voidType,
  writeStreamCarrier,
  writeStreamOptionsCarrier,
} from "../model.js";

import type {
  RustProviderConstantArgument,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustTargetTypeRef,
} from "../model.js";
export function fsModule(): RustProviderModuleDefinition {
  const m = "node:fs";
  const statsId = "node:fs::Stats";
  const makeDirectoryOptionsId = `${m}::MakeDirectoryOptions`;
  const rmOptionsId = `${m}::RmOptions`;
  const readStreamId = `${m}::ReadStream`;
  const writeStreamId = `${m}::WriteStream`;
  const watcherId = `${m}::FSWatcher`;
  const readStreamOptionsId = `${m}::ReadStreamOptions`;
  const writeStreamOptionsId = `${m}::WriteStreamOptions`;
  const bufferType = providerRef("node:buffer", "Buffer");
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.fs",
    imports: [
      { moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] },
      {
        moduleSpecifier: "node:http",
        namedImports: [{ exportedName: "ServerResponse" }],
      },
    ],
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
        id: makeDirectoryOptionsId,
        name: "MakeDirectoryOptions",
        kind: "interface" as const,
        members: [
          propertyMember(makeDirectoryOptionsId, "recursive", booleanType, {
            readonly: false,
            optional: true,
          }),
          propertyMember(makeDirectoryOptionsId, "mode", numberType, {
            readonly: false,
            optional: true,
          }),
        ],
      },
      {
        id: rmOptionsId,
        name: "RmOptions",
        kind: "interface" as const,
        members: [
          propertyMember(rmOptionsId, "recursive", booleanType, {
            readonly: false,
            optional: true,
          }),
          propertyMember(rmOptionsId, "force", booleanType, {
            readonly: false,
            optional: true,
          }),
          propertyMember(rmOptionsId, "maxRetries", numberType, {
            readonly: false,
            optional: true,
          }),
          propertyMember(rmOptionsId, "retryDelay", numberType, {
            readonly: false,
            optional: true,
          }),
        ],
      },
      {
        id: `${m}::mkdirSync`,
        name: "mkdirSync",
        kind: "function" as const,
        signatures: [
          { id: `${m}::mkdirSync(path)`, name: "mkdirSync", parameters: [{ name: "path", type: stringType }], returnType: voidType },
          { id: `${m}::mkdirSync(path,options)`, name: "mkdirSync", parameters: [{ name: "path", type: stringType }, { name: "options", type: providerRef(m, "MakeDirectoryOptions") }], returnType: voidType },
        ],
      },
      {
        id: `${m}::rmSync`,
        name: "rmSync",
        kind: "function" as const,
        signatures: [
          { id: `${m}::rmSync(path)`, name: "rmSync", parameters: [{ name: "path", type: stringType }], returnType: voidType },
          { id: `${m}::rmSync(path,options)`, name: "rmSync", parameters: [{ name: "path", type: stringType }, { name: "options", type: providerRef(m, "RmOptions") }], returnType: voidType },
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
      {
        id: readStreamOptionsId,
        name: "ReadStreamOptions",
        kind: "interface" as const,
        members: streamOptionMembers(readStreamOptionsId, ["r", "r+", "rs+"], true),
      },
      {
        id: writeStreamOptionsId,
        name: "WriteStreamOptions",
        kind: "interface" as const,
        members: [
          ...streamOptionMembers(
            writeStreamOptionsId,
            ["w", "wx", "w+", "wx+", "a", "ax", "a+", "ax+", "as", "as+"],
            false,
          ),
          propertyMember(writeStreamOptionsId, "flush", booleanType, {
            readonly: false,
            optional: true,
          }),
        ],
      },
      {
        id: readStreamId,
        name: "ReadStream",
        kind: "class" as const,
        members: [
          methodMember(readStreamId, "read", [], { kind: "union", types: [bufferType, { kind: "undefined" }] }),
          {
            id: `${readStreamId}.pipe`,
            name: "pipe",
            kind: "method" as const,
            signatures: [
              {
                id: `${readStreamId}.pipe(writeStream)`,
                parameters: [{
                  name: "destination",
                  type: providerRef(m, "WriteStream"),
                }],
                returnType: providerRef(m, "WriteStream"),
              },
              {
                id: `${readStreamId}.pipe(serverResponse)`,
                parameters: [{
                  name: "destination",
                  type: providerRef("node:http", "ServerResponse"),
                }],
                returnType: providerRef("node:http", "ServerResponse"),
              },
            ],
          },
          methodMember(readStreamId, "close", [], voidType),
          propertyMember(readStreamId, "path", stringType),
          propertyMember(readStreamId, "bytesRead", numberType),
        ],
      },
      {
        id: writeStreamId,
        name: "WriteStream",
        kind: "class" as const,
        members: [
          methodMember(writeStreamId, "write", [{ name: "chunk", type: bufferType }], booleanType),
          methodMember(writeStreamId, "close", [], voidType),
          propertyMember(writeStreamId, "path", stringType),
          propertyMember(writeStreamId, "bytesWritten", numberType),
        ],
      },
      {
        id: watcherId,
        name: "FSWatcher",
        kind: "class" as const,
        members: [
          methodMember(watcherId, "close", [], voidType),
          methodMember(watcherId, "ref", [], providerRef(m, "FSWatcher")),
          methodMember(watcherId, "unref", [], providerRef(m, "FSWatcher")),
          methodMember(watcherId, "hasRef", [], booleanType),
        ],
      },
      {
        id: `${m}::watch`,
        name: "watch",
        kind: "function" as const,
        signatures: [
          {
            id: `${m}::watch(path)`,
            parameters: [{ name: "path", type: stringType }],
            returnType: providerRef(m, "FSWatcher"),
          },
          {
            id: `${m}::watch(path,listener)`,
            parameters: [
              { name: "path", type: stringType },
              {
                name: "listener",
                type: {
                  kind: "function",
                  id: `${m}.WatchListener`,
                  parameters: [
                    { name: "eventType", type: stringType },
                    { name: "filename", type: stringType },
                  ],
                  returnType: voidType,
                },
              },
            ],
            returnType: providerRef(m, "FSWatcher"),
          },
        ],
      },
      {
        id: `${m}::watchFile`,
        name: "watchFile",
        kind: "function" as const,
        signatures: [{
          id: `${m}::watchFile(path,listener)`,
          parameters: [
            { name: "path", type: stringType },
            {
              name: "listener",
              type: {
                kind: "function",
                id: `${m}.StatWatcherListener`,
                parameters: [
                  { name: "current", type: providerRef(m, "Stats") },
                  { name: "previous", type: providerRef(m, "Stats") },
                ],
                returnType: voidType,
              },
            },
          ],
          returnType: voidType,
        }],
      },
      fnExport(m, "unwatchFile", [{ name: "path", type: stringType }], voidType),
      {
        id: `${m}::createReadStream`,
        name: "createReadStream",
        kind: "function" as const,
        signatures: [
          { id: `${m}::createReadStream(path)`, parameters: [{ name: "path", type: stringType }], returnType: providerRef(m, "ReadStream") },
          { id: `${m}::createReadStream(path,options)`, parameters: [{ name: "path", type: stringType }, { name: "options", type: providerRef(m, "ReadStreamOptions") }], returnType: providerRef(m, "ReadStream") },
        ],
      },
      {
        id: `${m}::createWriteStream`,
        name: "createWriteStream",
        kind: "function" as const,
        signatures: [
          { id: `${m}::createWriteStream(path)`, parameters: [{ name: "path", type: stringType }], returnType: providerRef(m, "WriteStream") },
          { id: `${m}::createWriteStream(path,options)`, parameters: [{ name: "path", type: stringType }, { name: "options", type: providerRef(m, "WriteStreamOptions") }], returnType: providerRef(m, "WriteStream") },
        ],
      },
    ],
  };
}

export function fsRows(): readonly RustProviderOperationDefinition[] {
  const statsId = "node:fs::Stats";
  const makeDirectoryOptionsId = "node:fs::MakeDirectoryOptions";
  const rmOptionsId = "node:fs::RmOptions";
  const readStreamId = "node:fs::ReadStream";
  const writeStreamId = "node:fs::WriteStream";
  const watcherId = "node:fs::FSWatcher";
  const readStreamOptionsId = "node:fs::ReadStreamOptions";
  const writeStreamOptionsId = "node:fs::WriteStreamOptions";
  const optionBool = rustOptionTargetType(boolCarrier);
  const optionNumber = rustOptionTargetType(float64Carrier);
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
      ...fallible("mkdirSync", "node_fs::mkdir_sync", { kind: "tuple", elements: [] }, [stringCarrier]),
      signatureId: "node:fs::mkdirSync(path)",
    },
    {
      exportId: "node:fs::mkdirSync",
      signatureId: "node:fs::mkdirSync(path,options)",
      operationKind: "method",
      target: { form: "call", path: "node_fs::mkdir_sync_with_options", argModes: ["ref", "value"] },
      resultCarrier: { kind: "tuple", elements: [] },
      parameterCarriers: [stringCarrier, makeDirectoryOptionsCarrier],
      ...providerNativeFallibility,
    },
    {
      ...fallible("rmSync", "node_fs::rm_sync", { kind: "tuple", elements: [] }, [stringCarrier]),
      signatureId: "node:fs::rmSync(path)",
    },
    {
      exportId: "node:fs::rmSync",
      signatureId: "node:fs::rmSync(path,options)",
      operationKind: "method",
      target: { form: "call", path: "node_fs::rm_sync_with_options", argModes: ["ref", "value"] },
      resultCarrier: { kind: "tuple", elements: [] },
      parameterCarriers: [stringCarrier, rmOptionsCarrier],
      ...providerNativeFallibility,
    },
    { exportId: makeDirectoryOptionsId, memberId: `${makeDirectoryOptionsId}.recursive`, operationKind: "property", target: { form: "field", name: "recursive" }, resultCarrier: optionBool, receiverCarrier: makeDirectoryOptionsCarrier },
    { exportId: makeDirectoryOptionsId, memberId: `${makeDirectoryOptionsId}.recursive`, operationKind: "property-set", target: { form: "field", name: "recursive" }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [optionBool], receiverCarrier: makeDirectoryOptionsCarrier },
    { exportId: makeDirectoryOptionsId, memberId: `${makeDirectoryOptionsId}.mode`, operationKind: "property", target: { form: "field", name: "mode" }, resultCarrier: optionNumber, receiverCarrier: makeDirectoryOptionsCarrier },
    { exportId: makeDirectoryOptionsId, memberId: `${makeDirectoryOptionsId}.mode`, operationKind: "property-set", target: { form: "field", name: "mode" }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [optionNumber], receiverCarrier: makeDirectoryOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.recursive`, operationKind: "property", target: { form: "field", name: "recursive" }, resultCarrier: optionBool, receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.recursive`, operationKind: "property-set", target: { form: "field", name: "recursive" }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [optionBool], receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.force`, operationKind: "property", target: { form: "field", name: "force" }, resultCarrier: optionBool, receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.force`, operationKind: "property-set", target: { form: "field", name: "force" }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [optionBool], receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.maxRetries`, operationKind: "property", target: { form: "field", name: "max_retries" }, resultCarrier: optionNumber, receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.maxRetries`, operationKind: "property-set", target: { form: "field", name: "max_retries" }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [optionNumber], receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.retryDelay`, operationKind: "property", target: { form: "field", name: "retry_delay_ms" }, resultCarrier: optionNumber, receiverCarrier: rmOptionsCarrier },
    { exportId: rmOptionsId, memberId: `${rmOptionsId}.retryDelay`, operationKind: "property-set", target: { form: "field", name: "retry_delay_ms" }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [optionNumber], receiverCarrier: rmOptionsCarrier },
    fallible("mkdtempSync", "node_fs::mkdtemp_sync", stringCarrier, [stringCarrier]),
    fallible("unlinkSync", "node_fs::unlink_sync", { kind: "tuple", elements: [] }, [stringCarrier]),
    fallible("symlinkSync", "node_fs::symlink_sync", { kind: "tuple", elements: [] }, [stringCarrier, stringCarrier]),
    fallible("copyFileSync", "node_fs::copy_file_sync", { kind: "tuple", elements: [] }, [stringCarrier, stringCarrier]),
    fallible("renameSync", "node_fs::rename_sync", { kind: "tuple", elements: [] }, [stringCarrier, stringCarrier]),
    fallible("realpathSync", "node_fs::realpath_sync", stringCarrier, [stringCarrier]),
    {
      ...fallible("watch", "node_fs::watch", fsWatcherCarrier, [stringCarrier]),
      signatureId: "node:fs::watch(path)",
    },
    {
      exportId: "node:fs::watch",
      signatureId: "node:fs::watch(path,listener)",
      operationKind: "method",
      target: { form: "call", path: "node_fs::watch_callable", argModes: ["ref", "value"] },
      resultCarrier: fsWatcherCarrier,
      parameterCarriers: [stringCarrier, fileWatchCallbackCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: "node:fs::watchFile",
      operationKind: "method",
      target: { form: "call", path: "node_fs::watch_file_callable", argModes: ["ref", "value"] },
      resultCarrier: { kind: "tuple", elements: [] },
      parameterCarriers: [stringCarrier, fileStatWatchCallbackCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: "node:fs::unwatchFile",
      operationKind: "method",
      target: { form: "call", path: "node_fs::unwatch_file", argModes: ["ref"] },
      resultCarrier: { kind: "tuple", elements: [] },
      parameterCarriers: [stringCarrier],
    },
    {
      ...fallible("createReadStream", "node_fs::create_read_stream", readStreamCarrier, [stringCarrier]),
      signatureId: "node:fs::createReadStream(path)",
    },
    {
      ...fallible("createReadStream", "node_fs::create_read_stream_with_options", readStreamCarrier, [stringCarrier, readStreamOptionsCarrier]),
      signatureId: "node:fs::createReadStream(path,options)",
    },
    {
      ...fallible("createWriteStream", "node_fs::create_write_stream", writeStreamCarrier, [stringCarrier]),
      signatureId: "node:fs::createWriteStream(path)",
    },
    {
      ...fallible("createWriteStream", "node_fs::create_write_stream_with_options", writeStreamCarrier, [stringCarrier, writeStreamOptionsCarrier]),
      signatureId: "node:fs::createWriteStream(path,options)",
    },
    ...streamOptionRows(readStreamOptionsId, readStreamOptionsCarrier, true),
    ...streamOptionRows(writeStreamOptionsId, writeStreamOptionsCarrier, false),
    { exportId: writeStreamOptionsId, memberId: `${writeStreamOptionsId}.flush`, operationKind: "property", target: { form: "field", name: "flush" }, resultCarrier: rustOptionTargetType(boolCarrier), receiverCarrier: writeStreamOptionsCarrier },
    { exportId: writeStreamOptionsId, memberId: `${writeStreamOptionsId}.flush`, operationKind: "property-set", target: { form: "field", name: "flush" }, resultCarrier: unitCarrier, parameterCarriers: [rustOptionTargetType(boolCarrier)], receiverCarrier: writeStreamOptionsCarrier },
    { exportId: readStreamId, memberId: `${readStreamId}.read`, operationKind: "method", target: { form: "receiver-method", name: "read", mutatesReceiver: true }, resultCarrier: rustOptionTargetType(bufferCarrier), receiverCarrier: readStreamCarrier, parameterCarriers: [], ...providerNativeFallibility },
    {
      exportId: readStreamId,
      memberId: `${readStreamId}.pipe`,
      signatureId: `${readStreamId}.pipe(writeStream)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "pipe_to", argModes: ["mut-ref"], mutatesReceiver: true },
      resultCarrier: { kind: "reference", referent: writeStreamCarrier, mutable: true },
      receiverCarrier: readStreamCarrier,
      parameterCarriers: [writeStreamCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: readStreamId,
      memberId: `${readStreamId}.pipe`,
      signatureId: `${readStreamId}.pipe(serverResponse)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "pipe_to", argModes: ["mut-ref"], mutatesReceiver: true },
      resultCarrier: { kind: "reference", referent: httpServerResponseCarrier, mutable: true },
      receiverCarrier: readStreamCarrier,
      parameterCarriers: [httpServerResponseCarrier],
      ...providerNativeFallibility,
    },
    { exportId: readStreamId, memberId: `${readStreamId}.close`, operationKind: "method", target: { form: "receiver-method", name: "close", mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, receiverCarrier: readStreamCarrier, parameterCarriers: [] },
    { exportId: readStreamId, memberId: `${readStreamId}.path`, operationKind: "property", target: { form: "field", name: "path" }, resultCarrier: stringCarrier, receiverCarrier: readStreamCarrier },
    { exportId: readStreamId, memberId: `${readStreamId}.bytesRead`, operationKind: "property", target: { form: "receiver-method", name: "bytes_read_number" }, resultCarrier: float64Carrier, receiverCarrier: readStreamCarrier },
    { exportId: writeStreamId, memberId: `${writeStreamId}.write`, operationKind: "method", target: { form: "receiver-method", name: "write", argModes: ["value"], mutatesReceiver: true }, resultCarrier: boolCarrier, receiverCarrier: writeStreamCarrier, parameterCarriers: [bufferCarrier], ...providerNativeFallibility },
    { exportId: writeStreamId, memberId: `${writeStreamId}.close`, operationKind: "method", target: { form: "receiver-method", name: "close", mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, receiverCarrier: writeStreamCarrier, parameterCarriers: [], ...providerNativeFallibility },
    { exportId: writeStreamId, memberId: `${writeStreamId}.path`, operationKind: "property", target: { form: "field", name: "path" }, resultCarrier: stringCarrier, receiverCarrier: writeStreamCarrier },
    { exportId: writeStreamId, memberId: `${writeStreamId}.bytesWritten`, operationKind: "property", target: { form: "receiver-method", name: "bytes_written_number" }, resultCarrier: float64Carrier, receiverCarrier: writeStreamCarrier },
    { exportId: watcherId, memberId: `${watcherId}.close`, operationKind: "method", target: { form: "receiver-method", name: "close", mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, receiverCarrier: fsWatcherCarrier, parameterCarriers: [] },
    { exportId: watcherId, memberId: `${watcherId}.ref`, operationKind: "method", target: { form: "receiver-method", name: "ref_", mutatesReceiver: true }, resultCarrier: { kind: "reference", referent: fsWatcherCarrier, mutable: true }, receiverCarrier: fsWatcherCarrier, parameterCarriers: [] },
    { exportId: watcherId, memberId: `${watcherId}.unref`, operationKind: "method", target: { form: "receiver-method", name: "unref", mutatesReceiver: true }, resultCarrier: { kind: "reference", referent: fsWatcherCarrier, mutable: true }, receiverCarrier: fsWatcherCarrier, parameterCarriers: [] },
    { exportId: watcherId, memberId: `${watcherId}.hasRef`, operationKind: "method", target: { form: "receiver-method", name: "has_ref" }, resultCarrier: boolCarrier, receiverCarrier: fsWatcherCarrier, parameterCarriers: [] },
    { exportId: statsId, memberId: `${statsId}.isFile`, operationKind: "method", target: { form: "receiver-method", name: "is_file" }, resultCarrier: boolCarrier },
    { exportId: statsId, memberId: `${statsId}.isDirectory`, operationKind: "method", target: { form: "receiver-method", name: "is_directory" }, resultCarrier: boolCarrier },
    { exportId: statsId, memberId: `${statsId}.isSymbolicLink`, operationKind: "method", target: { form: "receiver-method", name: "is_symbolic_link" }, resultCarrier: boolCarrier },
    { exportId: statsId, memberId: `${statsId}.size`, operationKind: "property", target: { form: "field", name: "size" }, resultCarrier: float64Carrier, resultConversion: rustUint64ToFloat64ValueConversion },
    { exportId: statsId, memberId: `${statsId}.mtimeMs`, operationKind: "property", target: { form: "receiver-method", name: "mtime_ms" }, resultCarrier: float64Carrier },
  ];
}

function streamOptionMembers(
  owner: string,
  flags: readonly string[],
  includeEnd: boolean,
) {
  const optional = { readonly: false, optional: true } as const;
  const literalFlags = {
    kind: "union" as const,
    types: flags.map((value) => ({ kind: "literal" as const, value })),
  };
  return [
    propertyMember(owner, "flags", literalFlags, optional),
    propertyMember(owner, "mode", numberType, optional),
    propertyMember(owner, "start", numberType, optional),
    ...(includeEnd ? [propertyMember(owner, "end", numberType, optional)] : []),
    propertyMember(owner, "highWaterMark", numberType, optional),
  ];
}

function streamOptionRows(
  owner: string,
  carrier: RustTargetTypeRef,
  includeEnd: boolean,
): readonly RustProviderOperationDefinition[] {
  const optionString = rustOptionTargetType(stringCarrier);
  const optionNumber = rustOptionTargetType(float64Carrier);
  const field = (
    sourceName: string,
    targetName: string,
    fieldCarrier: RustTargetTypeRef,
  ): readonly RustProviderOperationDefinition[] => [
    { exportId: owner, memberId: `${owner}.${sourceName}`, operationKind: "property", target: { form: "field", name: targetName }, resultCarrier: fieldCarrier, receiverCarrier: carrier },
    { exportId: owner, memberId: `${owner}.${sourceName}`, operationKind: "property-set", target: { form: "field", name: targetName }, resultCarrier: unitCarrier, parameterCarriers: [fieldCarrier], receiverCarrier: carrier },
  ];
  return [
    ...field("flags", "flags", optionString),
    ...field("mode", "mode", optionNumber),
    ...field("start", "start", optionNumber),
    ...(includeEnd ? field("end", "end", optionNumber) : []),
    ...field("highWaterMark", "high_water_mark", optionNumber),
  ];
}

// --- node:fs/promises --------------------------------------------------------
