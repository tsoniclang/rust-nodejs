import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";

const expectedModules = [
  "node:assert",
  "node:path",
  "node:os",
  "node:v8",
  "node:fs",
  "node:fs/promises",
  "node:process",
  "node:perf_hooks",
  "node:buffer",
  "node:child_process",
  "node:url",
  "node:crypto",
  "node:util",
  "node:http",
  "node:timers",
  "node:events",
  "node:stream",
  "node:dns",
  "node:dns/promises",
  "node:zlib",
  "node:net",
  "node:tls",
  "node:https",
  "node:readline",
  "node:worker_threads",
  "assert",
  "assert/strict",
  "node:assert/strict",
  "buffer",
  "child_process",
  "crypto",
  "fs",
  "fs/promises",
  "http",
  "os",
  "v8",
  "path",
  "process",
  "perf_hooks",
  "timers",
  "events",
  "stream",
  "dns",
  "dns/promises",
  "zlib",
  "net",
  "tls",
  "https",
  "readline",
  "worker_threads",
  "util",
  "url",
];

test("provider package declares the expected node module specifiers", () => {
  const plugin = createTsonicPlugin();
  const specifiers = plugin.moduleOwnership.map((ownership) => ownership.specifierPrefix);
  for (const moduleSpecifier of expectedModules) {
    assert.ok(specifiers.includes(moduleSpecifier), `missing module '${moduleSpecifier}'`);
  }
  assert.equal(specifiers.length, expectedModules.length);
});

test("provider package declares bare Node modules as canonical aliases", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  assert.deepEqual(contribution.definition.moduleAliases, [
    ["assert", "node:assert"],
    ["assert/strict", "node:assert"],
    ["node:assert/strict", "node:assert"],
    ["buffer", "node:buffer"],
    ["child_process", "node:child_process"],
    ["crypto", "node:crypto"],
    ["fs", "node:fs"],
    ["fs/promises", "node:fs/promises"],
    ["http", "node:http"],
    ["os", "node:os"],
    ["v8", "node:v8"],
    ["path", "node:path"],
    ["process", "node:process"],
    ["perf_hooks", "node:perf_hooks"],
    ["timers", "node:timers"],
    ["events", "node:events"],
    ["stream", "node:stream"],
    ["dns", "node:dns"],
    ["dns/promises", "node:dns/promises"],
    ["zlib", "node:zlib"],
    ["net", "node:net"],
    ["tls", "node:tls"],
    ["https", "node:https"],
    ["readline", "node:readline"],
    ["worker_threads", "node:worker_threads"],
    ["util", "node:util"],
    ["url", "node:url"],
  ].map(([moduleSpecifier, canonicalModuleSpecifier]) => ({
    moduleSpecifier,
    canonicalModuleSpecifier,
  })));
});

test("provider package contributes a non-empty operation row set", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  assert.equal(contribution.kind, "rust-provider-policy");
  assert.equal(contribution.definition.id, plugin.id);
  const rows = contribution.definition.operations;
  assert.ok(rows.length > 0);
  const readFileSync = rows.find((row) => row.exportId === "node:fs::readFileSync");
  assert.ok(readFileSync !== undefined, "missing node:fs::readFileSync row");
  assert.equal(readFileSync.isFallible, true);
  assert.equal(readFileSync.operationKind, "method");
});

test("native V8 flags retain an exact fallible string-to-void boundary", () => {
  const [contribution] = createTsonicPlugin().createTargetContributions({});
  const { modules, operations } = contribution.definition;
  const module = modules.find((entry) => entry.moduleSpecifier === "node:v8");
  assert.ok(module);
  assert.deepEqual(module.exports.map((entry) => entry.name), ["setFlagsFromString", "getHeapStatistics", "HeapInfo"]);
  const signature = module.exports[0].signatures[0];
  assert.deepEqual(signature.parameters, [{ name: "flags", type: { kind: "string" } }]);
  assert.deepEqual(signature.returnType, { kind: "void" });
  const rows = operations.filter((entry) => entry.exportId === "node:v8::setFlagsFromString");
  assert.equal(rows.length, 1);
  assert.equal(rows[0].isFallible, true);
  assert.equal(rows[0].errorBoundary, "provider-native");
  assert.equal(rows[0].errorCarrier.id, "rust.node.NodeError");
  assert.deepEqual(rows[0].target, {
    form: "call", path: "tsonic_rust_node::v8::set_flags_from_string", argModes: ["ref"],
  });
});

test("filesystem strings explicitly borrow str while byte buffers retain their native references", () => {
  const [contribution] = createTsonicPlugin().createTargetContributions({});
  const operations = contribution.definition.operations;
  const conversion = { kind: "semantic-conversion", id: "borrowed-str-from-owned-string" };
  for (const signature of [
    "statSync(path)", "statSync(path,options)", "lstatSync(path)",
    "readdirSync(path)", "mkdirSync(path)", "mkdirSync(path,options)",
    "rmSync(path)", "rmSync(path,options)", "readFileSync(path)",
    "writeFileSync(path,buffer)",
  ]) {
    const row = operations.find(entry => entry.signatureId === `node:fs::${signature}`);
    assert.ok(row, signature);
    assert.equal(row.target.argModes[0], "value", signature);
    assert.deepEqual(row.target.argConversions[0], conversion, signature);
  }
  for (const signature of ["statSync(bufferPath)", "readdirSync(bufferPath)", "mkdirSync(bufferPath)"]) {
    const row = operations.find(entry => entry.signatureId === `node:fs::${signature}`);
    assert.ok(row, signature);
    assert.equal(row.target.argModes[0], "ref", signature);
    assert.equal(row.target.argConversions?.[0], undefined, signature);
  }
  const writeBuffer = operations.find(entry => entry.signatureId === "node:fs::writeFileSync(path,buffer)");
  assert.equal(writeBuffer.target.argModes[1], "ref");
  assert.equal(writeBuffer.target.argConversions[1], undefined);
});

test("native V8 heap observations retain their exact result and fallible boundary", () => {
  const [contribution] = createTsonicPlugin().createTargetContributions({});
  const { modules, operations } = contribution.definition;
  const module = modules.find(entry => entry.moduleSpecifier === "node:v8");
  const heap = module.exports.find(entry => entry.name === "HeapInfo");
  const call = module.exports.find(entry => entry.name === "getHeapStatistics");
  assert.equal(heap.kind, "interface");
  assert.equal(heap.members.length, 15);
  assert.equal(new Set(heap.members.map(member => member.name)).size, 15);
  assert.deepEqual(call.signatures[0].parameters, []);
  assert.deepEqual(call.signatures[0].returnType, { kind: "provider-ref", moduleSpecifier: "node:v8", exportName: "HeapInfo" });
  const selected = operations.filter(entry => entry.exportId === "node:v8::getHeapStatistics");
  assert.equal(selected.length, 1);
  assert.equal(selected[0].resultCarrier.id, "rust.node.HeapInfo");
  assert.equal(selected[0].isFallible, true);
  assert.equal(selected[0].errorBoundary, "provider-native");
  for (const member of heap.members) {
    assert.equal(member.readonly, undefined);
    assert.equal(operations.filter(entry => entry.memberId === member.id && entry.operationKind === "property").length, 1);
    assert.equal(operations.filter(entry => entry.memberId === member.id && entry.operationKind === "property-set").length, 1);
  }
});

test("required Node capability families expose exact provider operations", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const { modules, operations } = contribution.definition;
  const required = [
    ["node:events", (row) => row.memberId === "node:events::EventEmitter.on" &&
      row.signatureId === "node:events::EventEmitter.on(1)"],
    ["node:stream", (row) => row.memberId === "node:stream::Readable.pipe" &&
      row.signatureId === "node:stream::Readable.pipe(destination)"],
    ["node:fs", (row) => row.exportId === "node:fs::watch" &&
      row.signatureId === "node:fs::watch(path,listener)"],
    ["node:https", (row) => row.exportId === "node:https::createServer"],
    ["node:zlib", (row) => row.exportId === "node:zlib::gzipSync"],
    ["node:net", (row) => row.exportId === "node:net::createConnection"],
    ["node:tls", (row) => row.exportId === "node:tls::connect" &&
      row.signatureId === "node:tls::connect(options,callback)"],
    ["node:dns", (row) => row.exportId === "node:dns::lookup"],
    ["node:dns/promises", (row) => row.exportId === "node:dns/promises::lookup" &&
      row.isAsync === true],
    ["node:readline", (row) => row.exportId === "node:readline::createInterface"],
    ["node:worker_threads", (row) =>
      row.memberId === "node:worker_threads::Worker.constructor" &&
      row.target.form === "source-module-construction"],
  ];

  for (const [moduleSpecifier, predicate] of required) {
    assert.ok(
      modules.some((module) => module.moduleSpecifier === moduleSpecifier),
      `missing provider module '${moduleSpecifier}'`,
    );
    assert.ok(
      operations.some(predicate),
      `missing exact operation for '${moduleSpecifier}'`,
    );
  }
});

test("provider type relations carry exact closed target carriers", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  assert.equal(contribution.kind, "rust-provider-policy");
  const { types, carrierPaths } = contribution.definition;
  const carrierId = (carrier) => carrier.kind === "target-specific" ? carrier.value.id : carrier.id;
  const namedRelations = types.map((relation) => {
    const carrier = relation.targetCarrier;
    if (carrier.kind !== "target-specific") return relation;
    assert.equal(carrier.target, "rust");
    assert.equal(carrier.name, "named-type");
    assert.equal(carrier.value.path, carrierPaths[carrier.value.id]);
    assert.deepEqual(carrier.value.genericArguments, []);
    assert.deepEqual(carrier.value.genericDefaults, []);
    assert.deepEqual(carrier.value.traits, { implementations: [] });
    return { ...relation, targetCarrier: { kind: "target-named", id: carrier.value.id } };
  });
  assert.deepEqual(namedRelations, [
    ["node:process::Process", "rust.node.Process"],
    ["node:child_process::SpawnSyncError", "rust.node.NodeError"],
    ["node:child_process::SpawnSyncOptionsWithBufferEncoding", "rust.node.SpawnSyncOptions", "struct-default"],
    ["node:perf_hooks::Performance", "rust.node.Performance"],
    ["node:process::CpuUsage", "rust.node.CpuUsage", "struct-default"],
    ["node:fs::Stats", "rust.node.Stats"],
    ["node:fs::StatOptions", "rust.node.StatOptions", "struct-default"],
    ["node:fs::BufferDirectoryOptions", "rust.node.BufferDirectoryOptions", "struct-default"],
    ["node:fs::BufferEncodingOptions", "rust.node.BufferEncodingOptions", "struct-default"],
    ["node:fs::FsConstants", "rust.node.FsConstants"],
    ["node:fs::Dirent", {
      kind: "target-named", id: "rust.node.Dirent",
      genericArguments: [{ kind: "type", type: { kind: "type-parameter", name: "Name" } }],
    }, undefined, [{ kind: "type", sourceName: "Name", defaultArgument: {
      kind: "type", type: { kind: "target-named", id: "rust.std.String" },
    } }]],
    ["node:fs::MakeDirectoryOptions", "rust.node.MakeDirectoryOptions", "struct-default"],
    ["node:fs::RmOptions", "rust.node.RmOptions", "struct-default"],
    ["node:process::ProcessEnv", "rust.node.ProcessEnv", "default"],
    ["node:process::MemoryUsage", "rust.node.MemoryUsage"],
    ["node:v8::HeapInfo", "rust.node.HeapInfo"],
    ["node:process::ProcessWriteStream", "rust.node.Writable"],
    ["node:buffer::Buffer", "rust.node.Buffer"],
    ["node:url::URL", "rust.node.Url"],
    ["node:url::UrlObject", "rust.node.UrlObject"],
    ["node:url::Url", "rust.node.UrlObject"],
    ["node:url::UrlWithStringQuery", "rust.node.UrlObject"],
    ["node:url::URLSearchParams", "rust.node.UrlSearchParams"],
    ["node:crypto::Hash", "rust.node.Hash"],
    ["node:crypto::Hmac", "rust.node.Hmac"],
    ["node:http::IncomingMessage", "rust.node.HttpIncomingMessage"],
    ["node:http::ServerResponse", "rust.node.HttpServerResponse"],
    ["node:http::Server", "rust.node.HttpServer"],
    ["node:http::IncomingHttpHeaders", "rust.node.IncomingHttpHeaders"],
    ["node:http::OutgoingHttpHeaders", "rust.node.OutgoingHttpHeaders"],
    ["node:http::AddressInfo", "rust.node.HttpAddressInfo"],
    ["node:http::ServerAddress", "rust.node.HttpServerAddress"],
    ["node:timers::Timeout", "rust.node.Timeout"],
    ["node:util::TextDecoder", "rust.node.TextDecoder"],
    ["node:events::EventEmitter", "rust.node.EventEmitter"],
    ["node:stream::Stream", "rust.node.Stream"],
    ["node:stream::Readable", "rust.node.Readable"],
    ["node:stream::Writable", "rust.node.Writable"],
    ["node:stream::Duplex", "rust.node.Duplex"],
    ["node:stream::Transform", "rust.node.Transform"],
    ["node:fs::ReadStream", "rust.node.ReadStream"],
    ["node:fs::WriteStream", "rust.node.WriteStream"],
    ["node:fs::ReadStreamOptions", "rust.node.ReadStreamOptions", "struct-default"],
    ["node:fs::WriteStreamOptions", "rust.node.WriteStreamOptions", "struct-default"],
    ["node:fs::FSWatcher", "rust.node.FsWatcher"],
    ["node:dns::LookupAddress", "rust.node.DnsLookupAddress"],
    ["node:zlib::ZlibOptions", "rust.node.ZlibOptions", "struct-default"],
    ["node:zlib::BrotliOptions", "rust.node.BrotliOptions", "struct-default"],
    ["node:zlib::ZlibTransform", "rust.node.ZlibTransform"],
    ["node:net::Socket", "rust.node.NetSocket"],
    ["node:net::Server", "rust.node.NetServer"],
    ["node:tls::ConnectionOptions", "rust.node.TlsConnectOptions", "struct-default"],
    ["node:tls::TlsOptions", "rust.node.TlsServerOptions", "struct-default"],
    ["node:tls::TLSSocket", "rust.node.TlsSocket"],
    ["node:tls::Server", "rust.node.TlsServer"],
    ["node:https::ServerOptions", "rust.node.TlsServerOptions", "struct-default"],
    ["node:https::Server", "rust.node.HttpsServer"],
    ["node:https::ClientRequest", "rust.node.HttpsClientRequest"],
    ["node:readline::ReadLineOptions", "rust.node.ReadlineOptions", "struct-default"],
    ["node:readline::Interface", "rust.node.ReadlineInterface"],
    ["node:worker_threads::Worker", "rust.node.Worker"],
    ["node:worker_threads::WorkerOptions", "rust.node.WorkerOptions", "struct-default"],
    ["node:worker_threads::MessagePort", "rust.node.MessagePort"],
    ["node:worker_threads::MessageChannel", "rust.node.MessageChannel"],
  ].map(([exportId, id, objectLiteralConstruction, genericParameters]) => ({
    exportId,
    targetCarrier: typeof id === "string" ? { kind: "target-named", id } : id,
    ...(genericParameters === undefined ? {} : { genericParameters }),
    ...(objectLiteralConstruction === undefined
      ? {}
      : { objectLiteralConstruction: { kind: objectLiteralConstruction } }),
  })));
  const expectedUpcasts = new Map([
    ["node:process::ProcessWriteStream", [["rust.node.Stream", "tsonic_rust_node::stream::writable_as_stream"]]],
    ["node:buffer::Buffer", [["rust.js.Uint8Array", "tsonic_rust_node::buffer::Buffer::as_uint8_array"]]],
    ["node:http::IncomingMessage", [["rust.node.Stream", "tsonic_rust_node::http::incoming_message_as_stream"], ["rust.node.Readable", "tsonic_rust_node::http::incoming_message_as_readable"]]],
    ["node:http::ServerResponse", [["rust.node.Stream", "tsonic_rust_node::http::server_response_as_stream"], ["rust.node.Writable", "tsonic_rust_node::http::server_response_as_writable"]]],
    ["node:stream::Readable", [["rust.node.Stream", "tsonic_rust_node::stream::readable_as_stream"]]],
    ["node:stream::Writable", [["rust.node.Stream", "tsonic_rust_node::stream::writable_as_stream"]]],
    ["node:stream::Duplex", [["rust.node.Stream", "tsonic_rust_node::stream::duplex_as_stream"], ["rust.node.Readable", "tsonic_rust_node::stream::duplex_as_readable"], ["rust.node.Writable", "tsonic_rust_node::stream::duplex_as_writable"]]],
    ["node:stream::Transform", [["rust.node.Stream", "tsonic_rust_node::stream::transform_as_stream"], ["rust.node.Readable", "tsonic_rust_node::stream::transform_as_readable"], ["rust.node.Writable", "tsonic_rust_node::stream::transform_as_writable"], ["rust.node.Duplex", "tsonic_rust_node::stream::transform_as_duplex"]]],
    ["node:fs::ReadStream", [["rust.node.Stream", "tsonic_rust_node::fs::read_stream_as_stream"], ["rust.node.Readable", "tsonic_rust_node::fs::read_stream_as_readable"]]],
    ["node:fs::WriteStream", [["rust.node.Stream", "tsonic_rust_node::fs::write_stream_as_stream"], ["rust.node.Writable", "tsonic_rust_node::fs::write_stream_as_writable"]]],
    ["node:zlib::ZlibTransform", [["rust.node.Stream", "tsonic_rust_node::zlib::zlib_as_stream"], ["rust.node.Readable", "tsonic_rust_node::zlib::zlib_as_readable"], ["rust.node.Writable", "tsonic_rust_node::zlib::zlib_as_writable"], ["rust.node.Duplex", "tsonic_rust_node::zlib::zlib_as_duplex"], ["rust.node.Transform", "tsonic_rust_node::zlib::zlib_as_transform"]]],
  ]);
  for (const relation of types) {
    const carrier = relation.targetCarrier;
    if (carrier.kind !== "target-specific") continue;
    assert.deepEqual(
      carrier.value.upcasts.map((upcast) => [carrierId(upcast.target), upcast.path]),
      expectedUpcasts.get(relation.exportId) ?? [],
      relation.exportId,
    );
  }
  assert.deepEqual(contribution.definition.carrierTraits["rust.node.Buffer"], {
    implementations: [
      {
        traitPath: "core::clone::Clone",
        requirements: [],
      },
      {
        traitPath: "tsonic_rust_js::value::JsClosedValueCarrier",
        requirements: [],
      },
    ],
  });
  assert.equal(
    Object.keys(contribution.definition.carrierTraits).every((id) =>
      contribution.definition.carrierPaths[id] !== undefined),
    true,
  );
});

test("provider package maps exact assert.ok overloads", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  assert.equal(contribution.kind, "rust-provider-policy");
  const rows = contribution.definition.operations.filter((row) => row.exportId === "node:assert::ok");
  assert.equal(rows.length, 2);
  assert.deepEqual(rows.map((row) => row.signatureId), [
    "node:assert::ok(value)",
    "node:assert::ok(value,message)",
  ]);
  assert.deepEqual(rows.map((row) => row.target.path), [
    "node_assert::ok",
    "node_assert::ok_with_message",
  ]);
  assert.equal(rows.every((row) => row.isFallible === true), true);
});

test("provider package maps legacy url parse to a fallible UrlObject row", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  assert.equal(contribution.kind, "rust-provider-policy");
  const rows = contribution.definition.operations;
  const parse = rows.find((row) => row.exportId === "node:url::parse");
  assert.ok(parse !== undefined, "missing node:url::parse row");
  assert.equal(parse.isFallible, true);
  assert.equal(parse.target.form, "call");
  assert.equal(parse.target.path, "node_url::parse_legacy");
  assert.deepEqual(parse.resultCarrier, { kind: "target-named", id: "rust.node.UrlObject" });
  const properties = ["href", "protocol", "host", "hostname", "port", "pathname", "search", "hash"];
  for (const name of properties) {
    const property = rows.find((row) => row.memberId === `node:url::UrlObject.${name}`);
    assert.ok(property !== undefined, `missing UrlObject property row '${name}'`);
    assert.equal(property.operationKind, "property");
    assert.equal(property.target.form, "receiver-method");
  }
  const format = rows.find((row) => row.exportId === "node:url::format");
  assert.ok(format !== undefined, "missing node:url::format row");
  assert.equal(format.target.path, "node_url::format_legacy");
  assert.equal(
    contribution.definition.carrierPaths["rust.node.UrlObject"],
    "tsonic_rust_node::url::LegacyUrlObject",
  );
});

test("provider package closes child-process and text-decoder operations", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const rows = contribution.definition.operations;

  const spawnSync = rows.find((row) => row.exportId === "node:child_process::spawnSync");
  assert.deepEqual(spawnSync, {
    exportId: "node:child_process::spawnSync",
    signatureId: "node:child_process::spawnSync(command,args)",
    operationKind: "method",
    target: {
      form: "call",
      path: "node_child_process::spawn_sync_result",
      argModes: ["ref", "ref"],
    },
    resultCarrier: { kind: "target-named", id: "rust.node.SpawnSyncResult" },
    parameterCarriers: [
      { kind: "target-named", id: "rust.std.String" },
      { kind: "type-parameter", name: "Arguments" },
    ],
    genericParameters: [{ kind: "type", sourceName: "Arguments" }],
    isFallible: true,
    errorBoundary: "provider-native",
    errorCarrier: { kind: "target-named", id: "rust.node.NodeError" },
  });
  const childProcessExports = contribution.definition.modules
    .find((module) => module.moduleSpecifier === "node:child_process")?.exports ?? [];
  const spawnReturns = childProcessExports.find((entry) =>
    entry.name === "SpawnSyncReturns"
  );
  assert.deepEqual(spawnReturns?.typeParameters, [{ name: "T" }]);
  assert.deepEqual(
    spawnReturns?.members?.map((member) => [member.name, member.type]),
    [
      ...["stdout", "stderr"].map(name => [name, { kind: "union", types: [{ kind: "type-parameter", name: "T" }, { kind: "literal", value: null }] }]),
      ["status", {
        kind: "union",
        types: [{ kind: "number" }, { kind: "literal", value: null }],
      }],
      ["pid", { kind: "number" }],
      ["signal", { kind: "union", types: [{ kind: "provider-ref", moduleSpecifier: "node:process", exportName: "Signals" }, { kind: "literal", value: null }] }],
      ["error", { kind: "provider-ref", moduleSpecifier: "node:child_process", exportName: "SpawnSyncError" }],
    ],
  );
  for (const name of ["stdout", "stderr", "status", "pid", "signal", "error"]) {
    assert.deepEqual(
      rows
        .filter((row) => row.memberId === `node:child_process::SpawnSyncReturns.${name}`)
        .map((row) => row.operationKind),
      ["property", "property-set"],
      `incomplete SpawnSyncReturns property '${name}'`,
    );
  }
  for (const name of ["message", "code"]) {
    const row = rows.find(row => row.memberId === `node:child_process::SpawnSyncError.${name}`);
    assert.deepEqual(row?.target, { form: "receiver-method", name });
    assert.deepEqual(row?.resultConversion, {
      kind: "semantic-conversion", id: "owned-string-from-borrowed-str",
    });
    assert.deepEqual(row?.receiverCarrier, { kind: "target-named", id: "rust.node.NodeError" });
  }

  const decode = rows.find((row) => row.memberId === "node:util::TextDecoder.decode");
  assert.equal(decode?.target.form, "receiver-method");
  assert.equal(decode?.target.name, "decode_buffer");
  assert.equal(decode?.isFallible, true);
  assert.equal(decode?.errorBoundary, "provider-native");
  const decoderConstructors = rows.filter((row) =>
    row.memberId === "node:util::TextDecoder.constructor"
  );
  assert.deepEqual(
    decoderConstructors.map((row) => row.signatureId),
    ["node:util::TextDecoder.constructor()"],
  );
  for (const name of ["encoding", "fatal", "ignoreBOM"]) {
    assert.ok(
      rows.some((row) => row.memberId === `node:util::TextDecoder.${name}`),
      `missing TextDecoder property '${name}'`,
    );
  }

  const urlExports = contribution.definition.modules
    .find((module) => module.moduleSpecifier === "node:url")?.exports ?? [];
  const urlObject = urlExports.find((entry) => entry.name === "UrlObject");
  assert.deepEqual(
    urlObject?.members?.map((member) => member.name),
    [
      "href",
      "protocol",
      "auth",
      "host",
      "hostname",
      "port",
      "pathname",
      "search",
      "query",
      "hash",
      "slashes",
    ],
  );
  assert.ok(urlObject?.members?.every((member) => member.optional === true));
  const optionalNullableString = {
    kind: "union",
    types: [
      { kind: "string" },
      { kind: "literal", value: null },
      { kind: "undefined" },
    ],
  };
  for (const member of urlObject?.members ?? []) {
    assert.equal(member.readonly, undefined);
    assert.deepEqual(
      member.type,
      member.name === "slashes"
        ? {
          kind: "union",
          types: [
            { kind: "boolean" },
            { kind: "literal", value: null },
            { kind: "undefined" },
          ],
        }
        : optionalNullableString,
    );
  }
  const legacyUrl = urlExports.find((entry) => entry.name === "Url");
  assert.equal(legacyUrl?.heritage, undefined);
  assert.deepEqual(legacyUrl?.members?.map((member) => member.name), [
    "href",
    "protocol",
    "auth",
    "host",
    "hostname",
    "port",
    "pathname",
    "search",
    "query",
    "hash",
    "path",
    "slashes",
  ]);
  assert.deepEqual(
    legacyUrl?.members?.find((member) => member.name === "href")?.type,
    { kind: "string" },
  );
  assert.deepEqual(
    legacyUrl?.members?.find((member) => member.name === "pathname")?.type,
    {
      kind: "union",
      types: [{ kind: "string" }, { kind: "literal", value: null }],
    },
  );
  const stringQueryUrl = urlExports.find((entry) => entry.name === "UrlWithStringQuery");
  assert.deepEqual(stringQueryUrl?.heritage, [{
    kind: "extends",
    type: {
      kind: "provider-ref",
      moduleSpecifier: "node:url",
      exportName: "Url",
    },
  }]);
  assert.deepEqual(stringQueryUrl?.members?.map((member) => member.name), ["query"]);
  const urlMemberGroups = [
    ["UrlObject", urlObject?.members?.map((member) => member.name) ?? []],
    ["Url", legacyUrl?.members?.map((member) => member.name) ?? []],
    ["UrlWithStringQuery", ["query"]],
  ];
  for (const [exportName, memberNames] of urlMemberGroups) {
    const exportId = `node:url::${exportName}`;
    for (const name of memberNames) {
      assert.deepEqual(
        rows
          .filter((row) => row.exportId === exportId && row.memberId === `${exportId}.${name}`)
          .map((row) => row.operationKind),
        ["property", "property-set"],
        `incomplete ${exportName} property '${name}'`,
      );
    }
  }
});

test("provider package maps util format to the generic value-slice call form", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  assert.equal(contribution.kind, "rust-provider-policy");
  const rows = contribution.definition.operations;
  const format = rows.find((row) => row.exportId === "node:util::format");
  assert.ok(format !== undefined, "missing node:util::format row");
  assert.deepEqual(format.target, {
    form: "call-value-slice",
    path: "node_util::format",
    leadingArguments: [{ carrier: { kind: "target-named", id: "rust.std.String" }, mode: "ref" }],
    elementCarrier: { kind: "target-named", id: "rust.js.JsValue" },
  });
  assert.equal(format.isFallible, true);
  assert.equal(format.errorBoundary, "provider-native");
});

test("provider package maps process execPath to a fallible property row", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  assert.equal(contribution.kind, "rust-provider-policy");
  const rows = contribution.definition.operations;
  const execPath = rows.find((row) => row.exportId === "node:process::execPath");
  assert.ok(execPath !== undefined, "missing node:process::execPath row");
  assert.equal(execPath.operationKind, "property");
  assert.equal(execPath.isFallible, true);
  assert.equal(execPath.target.path, "node_process::exec_path");
});

test("provider package maps process argv through the fallible native snapshot", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const rows = contribution.definition.operations.filter((row) =>
    row.exportId === "node:process::argv" || row.memberId === "node:process::Process.argv");
  assert.equal(rows.length, 2);
  for (const row of rows) {
    assert.equal(row.operationKind, "property");
    assert.equal(row.isFallible, true);
    assert.equal(row.errorBoundary, "provider-native");
    assert.equal(row.target.path, "node_process::argv");
  }
});

test("provider package exposes exact process env absence and writable exit status", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const processModule = contribution.definition.modules.find((module) =>
    module.moduleSpecifier === "node:process");
  assert.ok(processModule !== undefined);
  const processEnv = processModule.exports.find((entry) => entry.id === "node:process::ProcessEnv");
  assert.ok(processEnv !== undefined && processEnv.kind === "interface");
  assert.deepEqual(processEnv.members[0].signatures[0].returnType, {
    kind: "union",
    types: [{ kind: "string" }, { kind: "undefined" }],
  });
  const exitCode = processModule.exports.find((entry) => entry.id === "node:process::exitCode");
  assert.deepEqual(exitCode?.type, {
    kind: "union",
    types: [{ kind: "number" }, { kind: "literal", value: null }],
  });
  const defaultObject = processModule.exports.find((entry) => entry.exportKind === "default");
  assert.equal(defaultObject?.kind, "value");
  assert.deepEqual(defaultObject?.type, { kind: "provider-ref", moduleSpecifier: "node:process", exportName: "Process" });
  const processType = processModule.exports.find(entry => entry.id === "node:process::Process");
  assert.equal(processType?.kind, "interface");
  const defaultExitCode = processType.members.find((member) => member.name === "exitCode");
  assert.equal(defaultExitCode?.readonly, undefined);
  assert.equal(defaultExitCode?.static, undefined);
  const defaultArgv = processType.members.find((member) => member.name === "argv");
  assert.equal(defaultArgv?.readonly, true);
  const rows = contribution.definition.operations.filter((row) =>
    row.memberId === "node:process::Process.exitCode");
  assert.deepEqual(rows.map((row) => [row.operationKind, row.target.path]), [
    ["property", "node_process::exit_code"],
    ["property-set", "node_process::set_exit_code"],
  ]);
  const selectedValueRows = contribution.definition.operations.filter((row) =>
    row.exportId === "node:process::exitCode" && row.memberId === undefined);
  assert.deepEqual(selectedValueRows.map((row) => [row.operationKind, row.target.path]), [
    ["property", "node_process::exit_code"],
    ["property-set", "node_process::set_exit_code"],
  ]);
});

test("provider package closes process identity, timing, and memory contracts", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const processModule = contribution.definition.modules.find((module) =>
    module.moduleSpecifier === "node:process");
  assert.ok(processModule !== undefined);

  for (const name of [
    "availableMemory", "chdir", "constrainedMemory", "hrtime", "memoryUsage", "uptime", "argv0", "version",
  ]) {
    assert.ok(processModule.exports.some((entry) => entry.name === name), `missing process export '${name}'`);
  }
  const memoryUsage = processModule.exports.find((entry) => entry.id === "node:process::MemoryUsage");
  assert.ok(memoryUsage !== undefined && memoryUsage.kind === "class");
  assert.deepEqual(memoryUsage.members.map((member) => member.name), [
    "rss", "heapTotal", "heapUsed", "external", "arrayBuffers",
  ]);

  const rows = contribution.definition.operations;
  assert.deepEqual(
    rows.filter((row) => row.exportId === "node:process::hrtime").map((row) => [row.signatureId, row.target.path]),
    [
      ["node:process::hrtime()", "node_process::hrtime_open"],
      ["node:process::hrtime(previous)", "node_process::hrtime_since"],
    ],
  );
  assert.deepEqual(
    rows.filter((row) => row.memberId === "node:process::Process.hrtime").map((row) => [row.signatureId, row.target.path]),
    [
      ["node:process::Process.hrtime()", "node_process::hrtime_open"],
      ["node:process::Process.hrtime(previous)", "node_process::hrtime_since"],
    ],
  );

  const namedMethods = ["availableMemory", "chdir", "constrainedMemory", "memoryUsage", "uptime"];
  for (const name of namedMethods) {
    const named = rows.find((row) => row.exportId === `node:process::${name}`);
    const defaultMember = rows.find((row) => row.memberId === `node:process::Process.${name}`);
    assert.ok(named !== undefined, `missing named process row '${name}'`);
    assert.ok(defaultMember !== undefined, `missing default process row '${name}'`);
    assert.deepEqual(defaultMember.target, named.target);
    assert.deepEqual(defaultMember.resultCarrier, named.resultCarrier);
  }
  for (const name of ["argv0", "version"]) {
    const named = rows.find((row) => row.exportId === `node:process::${name}`);
    const defaultMember = rows.find((row) => row.memberId === `node:process::Process.${name}`);
    assert.ok(named !== undefined, `missing named process property '${name}'`);
    assert.ok(defaultMember !== undefined, `missing default process property '${name}'`);
    assert.deepEqual(defaultMember.target, named.target);
  }

  const fieldNames = new Map([
    ["rss", "rss"],
    ["heapTotal", "heap_total"],
    ["heapUsed", "heap_used"],
    ["external", "external"],
    ["arrayBuffers", "array_buffers"],
  ]);
  for (const [sourceName, targetName] of fieldNames) {
    const row = rows.find((candidate) =>
      candidate.memberId === `node:process::MemoryUsage.${sourceName}`);
    assert.deepEqual(row?.target, { form: "field", name: targetName });
    assert.equal(row?.resultConversion, undefined);
    assert.deepEqual(row?.resultCarrier, { kind: "source-primitive", name: "uint64" });
  }
  assert.equal(
    contribution.definition.carrierPaths["rust.node.MemoryUsage"],
    "tsonic_rust_node::process::MemoryUsage",
  );
  assert.deepEqual(
    contribution.definition.carrierTraits["rust.node.MemoryUsage"],
    {
      implementations: [{
        traitPath: "core::clone::Clone",
        requirements: [],
      }],
    },
  );
});

test("provider package closes process stdout and stderr output contracts", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const processModule = contribution.definition.modules.find((module) =>
    module.moduleSpecifier === "node:process");
  assert.ok(processModule !== undefined);
  assert.deepEqual(processModule.imports, [
    {
      moduleSpecifier: "node:buffer",
      namedImports: [{ exportedName: "Buffer" }],
    },
    {
      moduleSpecifier: "node:stream",
      namedImports: [{ exportedName: "Readable" }],
    },
  ]);
  const stream = processModule.exports.find((entry) =>
    entry.id === "node:process::ProcessWriteStream");
  assert.ok(stream !== undefined && stream.kind === "class");
  assert.deepEqual(stream.members.map((member) => member.name), ["write", "isTTY", "fd"]);
  assert.deepEqual(stream.members[0].signatures.map((signature) => signature.id), [
    "node:process::ProcessWriteStream.write(string)",
    "node:process::ProcessWriteStream.write(buffer)",
  ]);
  for (const name of ["stdout", "stderr"]) {
    const named = processModule.exports.find((entry) => entry.id === `node:process::${name}`);
    assert.deepEqual(named?.type, {
      kind: "provider-ref",
      moduleSpecifier: "node:process",
      exportName: "ProcessWriteStream",
    });
    const namedRow = contribution.definition.operations.find((row) =>
      row.exportId === `node:process::${name}` && row.memberId === undefined);
    const defaultRow = contribution.definition.operations.find((row) =>
      row.memberId === `node:process::Process.${name}`);
    assert.equal(namedRow?.target.path, `node_process::${name}`);
    assert.deepEqual(defaultRow?.target, namedRow?.target);
    assert.deepEqual(defaultRow?.resultCarrier, namedRow?.resultCarrier);
  }
  const writeRows = contribution.definition.operations.filter((row) =>
    row.memberId === "node:process::ProcessWriteStream.write");
  assert.deepEqual(writeRows.map((row) => [row.signatureId, row.target.name]), [
    ["node:process::ProcessWriteStream.write(string)", "write_string"],
    ["node:process::ProcessWriteStream.write(buffer)", "write_buffer"],
  ]);
  assert.equal(writeRows.every((row) => row.isFallible === true), true);
  assert.equal(
    contribution.definition.carrierPaths["rust.node.Writable"],
    "tsonic_rust_node::stream::Writable",
  );
});

test("provider package exposes exact filesystem and path contracts required by portable applications", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const { modules, operations } = contribution.definition;
  const path = modules.find((module) => module.moduleSpecifier === "node:path");
  const fs = modules.find((module) => module.moduleSpecifier === "node:fs");
  assert.ok(path !== undefined);
  assert.ok(fs !== undefined);
  assert.ok(path.exports.some((entry) => entry.id === "node:path::relative"));
  assert.ok(path.exports.some((entry) => entry.id === "node:path::sep" && entry.kind === "value"));
  const separator = operations.find((row) => row.exportId === "node:path::sep");
  assert.deepEqual(separator?.resultConversion, {
    kind: "semantic-conversion",
    id: "owned-string-from-borrowed-str",
  });
  assert.deepEqual(separator?.target, { form: "call", path: "node_path::sep" });
  assert.ok(fs.exports.some((entry) => entry.id === "node:fs::mkdtempSync"));
  assert.ok(fs.exports.some((entry) => entry.id === "node:fs::symlinkSync"));
  const stats = fs.exports.find((entry) => entry.id === "node:fs::Stats");
  assert.ok(stats !== undefined && stats.kind === "class");
  assert.ok(stats.members.some((member) => member.id === "node:fs::Stats.isSymbolicLink"));
  assert.ok(stats.members.some((member) => member.id === "node:fs::Stats.mtimeMs"));
  assert.deepEqual(
    operations.filter((row) => row.exportId === "node:fs::readFileSync").map((row) => row.signatureId),
    ["node:fs::readFileSync(path)", "node:fs::readFileSync(path,encoding)"],
  );
  assert.deepEqual(
    operations.filter((row) => row.exportId === "node:fs::writeFileSync").map((row) => row.signatureId),
    ["node:fs::writeFileSync(path,data,encoding)", "node:fs::writeFileSync(path,buffer)"],
  );
  const symlink = operations.find((row) => row.exportId === "node:fs::symlinkSync");
  assert.deepEqual(symlink?.target, {
    form: "call",
    path: "node_fs::symlink_sync",
    argModes: ["value", "value"],
    argConversions: [
      { kind: "semantic-conversion", id: "borrowed-str-from-owned-string" },
      { kind: "semantic-conversion", id: "borrowed-str-from-owned-string" },
    ],
  });
  assert.equal(symlink?.isFallible, true);
});

test("provider package preserves fluent hash identity for string and buffer updates", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const rows = contribution.definition.operations.filter((row) =>
    row.memberId === "node:crypto::Hash.update");
  assert.deepEqual(rows.map((row) => row.signatureId), [
    "node:crypto::Hash.update(string)",
    "node:crypto::Hash.update(buffer)",
  ]);
  assert.deepEqual(rows.map((row) => row.resultCarrier), [
    { kind: "target-named", id: "rust.node.Hash" },
    { kind: "target-named", id: "rust.node.Hash" },
  ]);
  assert.deepEqual(rows.map((row) => row.target.name), ["update_str_owned", "update_buffer_owned"]);
});

test("provider package maps Buffer.from overloads by exact selected signature", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const rows = contribution.definition.operations.filter((row) =>
    row.memberId === "node:buffer::Buffer.from");
  assert.deepEqual(rows.map((row) => row.signatureId), [
    "node:buffer::Buffer.from(buffer)",
    "node:buffer::Buffer.from(string)",
    "node:buffer::Buffer.from(string,encoding)",
    "node:buffer::Buffer.from(numberArray)",
  ]);
  assert.deepEqual(rows.map((row) => row.target.path), [
    "node_buffer::Buffer::copy_from_buffer",
    "node_buffer::Buffer::from_string",
    "node_buffer::Buffer::from_string_enc",
    "node_buffer::Buffer::from_number_array",
  ]);
});

test("provider package closes Buffer views, copies, swaps, and numeric operations", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const bufferModule = contribution.definition.modules.find((module) =>
    module.moduleSpecifier === "node:buffer");
  const buffer = bufferModule?.exports.find((entry) => entry.id === "node:buffer::Buffer");
  assert.ok(buffer !== undefined && buffer.kind === "class");

  const copy = buffer.members.find((member) => member.id === "node:buffer::Buffer.copy");
  assert.deepEqual(copy.signatures.map((signature) => signature.id), [
    "node:buffer::Buffer.copy(target)",
    "node:buffer::Buffer.copy(target,targetStart)",
    "node:buffer::Buffer.copy(target,targetStart,sourceStart)",
    "node:buffer::Buffer.copy(target,targetStart,sourceStart,sourceEnd)",
  ]);

  for (const name of ["slice", "subarray"]) {
    const member = buffer.members.find((candidate) => candidate.name === name);
    assert.deepEqual(member.signatures.map((signature) => signature.id), [
      `node:buffer::Buffer.${name}()`,
      `node:buffer::Buffer.${name}(start)`,
      `node:buffer::Buffer.${name}(start,end)`,
    ]);
  }

  const numericNames = [
    "readUInt8", "readInt8", "readUInt16LE", "readUInt16BE", "readInt16LE", "readInt16BE",
    "readUInt32LE", "readUInt32BE", "readInt32LE", "readInt32BE", "readFloatLE", "readFloatBE",
    "readDoubleLE", "readDoubleBE", "writeUInt8", "writeInt8", "writeUInt16LE", "writeUInt16BE",
    "writeInt16LE", "writeInt16BE", "writeUInt32LE", "writeUInt32BE", "writeInt32LE", "writeInt32BE",
    "writeFloatLE", "writeFloatBE", "writeDoubleLE", "writeDoubleBE",
  ];
  for (const name of numericNames) {
    const member = buffer.members.find((candidate) => candidate.name === name);
    assert.equal(member.signatures.length, 2, `${name} must expose default and explicit offsets`);
  }

  const rows = contribution.definition.operations;
  assert.equal(rows.filter((row) => numericNames.some((name) =>
    row.memberId === `node:buffer::Buffer.${name}`)).length, numericNames.length * 2);
  const writeUInt8 = rows.filter((row) => row.memberId === "node:buffer::Buffer.writeUInt8");
  assert.deepEqual(writeUInt8.map((row) => row.resultCarrier), [
    { kind: "source-primitive", name: "native-uint" },
    { kind: "source-primitive", name: "native-uint" },
  ]);
  assert.deepEqual(writeUInt8.map((row) => row.target.receiverMode), ["mut-ref", "mut-ref"]);
  assert.deepEqual(writeUInt8[0].target.trailingArguments, [{ kind: "integer", value: 0 }]);

  const copyRows = rows.filter((row) => row.memberId === "node:buffer::Buffer.copy");
  assert.equal(copyRows.length, 4);
  assert.equal(copyRows[0].target.argModes[0], "ref");
  for (const name of ["swap16", "swap32", "swap64"]) {
    const row = rows.find((candidate) => candidate.memberId === `node:buffer::Buffer.${name}`);
    assert.deepEqual(row.target, { form: "receiver-method", name, mutatesReceiver: true });
    assert.equal(row.isFallible, true);
  }
});

test("provider package maps HTTP server mutation and lifecycle contracts exactly", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const { operations, binaryHooks, carrierPaths } = contribution.definition;

  const statusRead = operations.find((row) =>
    row.memberId === "node:http::ServerResponse.statusCode" && row.operationKind === "property");
  const statusWrite = operations.find((row) =>
    row.memberId === "node:http::ServerResponse.statusCode" && row.operationKind === "property-set");
  assert.deepEqual(statusRead?.target, { form: "receiver-method", name: "status_code" });
  assert.deepEqual(statusWrite?.target, { form: "receiver-method", name: "set_status_code", argModes: ["value"] });
  assert.deepEqual(statusWrite?.parameterCarriers, [{ kind: "source-primitive", name: "int32" }]);

  const endRows = operations.filter((row) => row.memberId === "node:http::ServerResponse.end");
  assert.deepEqual(endRows.map((row) => row.signatureId), [
    "node:http::ServerResponse.end()",
    "node:http::ServerResponse.end(string)",
    "node:http::ServerResponse.end(buffer)",
  ]);
  assert.deepEqual(endRows.map((row) => row.target.name), ["end_empty", "end_string", "end_buffer"]);

  const listenRows = operations.filter((row) => row.memberId === "node:http::Server.listen");
  assert.deepEqual(listenRows.map((row) => row.target.name), [
    "listen_default_host_optional", "listen_optional", "listen_with_backlog_optional", "listen_path_optional",
  ]);
  assert.equal(listenRows.every((row) => row.isFallible === true), true);
  assert.deepEqual(listenRows.map((row) => row.immediateCallback), [undefined, undefined, undefined, undefined]);
  const createServer = operations.find((row) => row.exportId === "node:http::createServer");
  assert.equal(createServer?.immediateCallback, undefined);
  assert.deepEqual(carrierPaths["rust.node.HttpServerResponse"],
    "tsonic_rust_node::http::ServerResponse");
  assert.deepEqual(binaryHooks, [
    {
      id: "node-performance-clock",
      phase: "before-initialization",
      path: "tsonic_rust_node::perf_hooks::initialize_clock",
      requiredCrate: "tsonic_rust_node",
    },
    {
      id: "node-event-loop",
      phase: "after-entry",
      path: "tsonic_rust_node::run_event_loop",
      requiredCrate: "tsonic_rust_node",
      isFallible: true,
      errorBoundary: "source-program",
    },
    {
      id: "node-process-exit-code",
      phase: "after-entry",
      path: "tsonic_rust_node::process::apply_exit_code",
      requiredCrate: "tsonic_rust_node",
    },
  ]);
});

test("distinct incoming headers select one native indexer without rebuilding storage", () => {
  const [contribution] = createTsonicPlugin().createTargetContributions({});
  const { modules, operations } = contribution.definition;
  const http = modules.find((entry) => entry.moduleSpecifier === "node:http");
  assert.ok(http);
  const values = http.exports.find((entry) => entry.name === "IncomingHttpHeaderValues");
  assert.deepEqual(values?.members, [{
    id: "node:http::IncomingHttpHeaderValues.indexer",
    name: "indexer",
    kind: "indexer",
    signatures: [{
      id: "node:http::IncomingHttpHeaderValues.indexer(name)",
      parameters: [{ name: "name", type: { kind: "string" } }],
      returnType: { kind: "union", types: [
        { kind: "array", elementType: { kind: "string" } },
        { kind: "undefined" },
      ] },
    }],
  }]);
  const incoming = http.exports.find((entry) => entry.name === "IncomingMessage");
  assert.deepEqual(incoming?.members.find((member) => member.name === "headersDistinct")?.type, {
    kind: "provider-ref", moduleSpecifier: "node:http", exportName: "IncomingHttpHeaderValues",
  });
  const rows = operations.filter((row) =>
    row.memberId === "node:http::IncomingHttpHeaderValues.indexer"
  );
  assert.equal(rows.length, 1);
  assert.equal(rows[0].operationKind, "indexer");
  assert.deepEqual(rows[0].target, { form: "receiver-method", name: "get_values", argModes: ["ref"] });
  assert.equal(rows[0].isFallible, true);
});

test("provider package maps timers to the shared Node event loop", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const rows = contribution.definition.operations.filter((candidate) =>
    candidate.exportId === "node:timers::setTimeout" ||
    candidate.exportId === "node:timers::setInterval");
  assert.deepEqual(rows.map((row) => row.exportId), [
    "node:timers::setTimeout",
    "node:timers::setInterval",
  ]);
  assert.deepEqual(rows.map((row) => row.target), [
    { form: "call", path: "node_timers::set_timeout_callable" },
    { form: "call", path: "node_timers::set_interval_callable" },
  ]);
  assert.equal(rows.every((row) => row.operationKind === "method"), true);
  assert.equal(rows.every((row) => row.immediateCallback === undefined), true);
  assert.equal(rows.every((row) =>
    JSON.stringify(row.resultCarrier) === JSON.stringify({ kind: "target-named", id: "rust.node.Timeout" })), true);
});
