import assert from "node:assert/strict";
import test from "node:test";
import {
  emptyRustGenerics,
  rustCloneTrait,
  rustCopyTrait,
  rustDefaultTrait,
  rustJsArrayTargetType,
  rustJsValueTargetType,
  rustProviderPathTargetType,
  rustProviderTypeIdentity,
  rustStringTargetType,
} from "@tsonic/target-rust/provider";
import { createTsonicPlugin } from "../../dist/index.js";

const providerOwner = {
  packageId: "@tsonic/rust-nodejs",
  packageVersion: "0.0.1",
  compilationSnapshotId: "@tsonic/rust-nodejs@0.0.1",
};
const cloneTraits = [{
  trait: rustCloneTrait,
  genericBindings: [],
  requirements: [],
}];
const cloneCopyTraits = [
  { trait: rustCloneTrait, genericBindings: [], requirements: [] },
  { trait: rustCopyTrait, genericBindings: [], requirements: [] },
];
const cloneCopyDefaultTraits = [
  { trait: rustCloneTrait, genericBindings: [], requirements: [] },
  { trait: rustDefaultTrait, genericBindings: [], requirements: [] },
  { trait: rustCopyTrait, genericBindings: [], requirements: [] },
];
const cloneDefaultTraits = [
  { trait: rustCloneTrait, genericBindings: [], requirements: [] },
  { trait: rustDefaultTrait, genericBindings: [], requirements: [] },
];

function nodeCarrier(itemId, displayPath) {
  return rustProviderPathTargetType({
    owner: providerOwner,
    itemId,
    displayPath,
  });
}

function nodeTraitContract(itemId, implementations = cloneTraits) {
  return {
    typeIdentity: rustProviderTypeIdentity(providerOwner, itemId),
    contract: { implementations },
  };
}

const expectedTraitContracts = [
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

function nodeStructType(exportId, itemId, displayPath, options = {}) {
  return {
    exportId,
    targetDeclarationKind: "struct",
    sourceGenericBindings: [],
    targetGenerics: emptyRustGenerics,
    targetCarrier: nodeCarrier(itemId, displayPath),
    ...(options.objectLiteralConstruction === true
      ? { objectLiteralConstruction: { kind: "struct-default" } }
      : {}),
  };
}

const expectedModules = [
  "node:assert",
  "node:path",
  "node:os",
  "node:fs",
  "node:fs/promises",
  "node:process",
  "node:buffer",
  "node:child_process",
  "node:url",
  "node:crypto",
  "node:util",
  "node:http",
  "node:timers",
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
  "path",
  "process",
  "timers",
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
    ["path", "node:path"],
    ["process", "node:process"],
    ["timers", "node:timers"],
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

test("provider type relations carry exact closed target carriers", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  assert.equal(contribution.kind, "rust-provider-policy");
  assert.deepEqual(contribution.definition.types, [
    nodeStructType("node:fs::Stats", "rust.node.Stats", "tsonic_rust_node::fs::Stats"),
    nodeStructType(
      "node:fs::MakeDirectoryOptions",
      "rust.node.MakeDirectoryOptions",
      "tsonic_rust_node::fs::MakeDirectoryOptions",
      { objectLiteralConstruction: true },
    ),
    nodeStructType(
      "node:fs::RmOptions",
      "rust.node.RmOptions",
      "tsonic_rust_node::fs::RmOptions",
      { objectLiteralConstruction: true },
    ),
    nodeStructType(
      "node:process::ProcessEnv",
      "rust.node.ProcessEnv",
      "tsonic_rust_node::process::ProcessEnv",
    ),
    nodeStructType(
      "node:process::MemoryUsage",
      "rust.node.MemoryUsage",
      "tsonic_rust_node::process::MemoryUsage",
    ),
    nodeStructType(
      "node:process::ProcessWriteStream",
      "rust.node.ProcessWriteStream",
      "tsonic_rust_node::process::ProcessWriteStream",
    ),
    nodeStructType("node:buffer::Buffer", "rust.node.Buffer", "tsonic_rust_node::buffer::Buffer"),
    nodeStructType("node:url::URL", "rust.node.Url", "tsonic_rust_node::url::Url"),
    nodeStructType(
      "node:url::UrlObject",
      "rust.node.UrlObject",
      "tsonic_rust_node::url::LegacyUrlObject",
    ),
    nodeStructType(
      "node:url::Url",
      "rust.node.UrlObject",
      "tsonic_rust_node::url::LegacyUrlObject",
    ),
    nodeStructType(
      "node:url::UrlWithStringQuery",
      "rust.node.UrlObject",
      "tsonic_rust_node::url::LegacyUrlObject",
    ),
    nodeStructType(
      "node:url::URLSearchParams",
      "rust.node.UrlSearchParams",
      "tsonic_rust_node::url::UrlSearchParams",
    ),
    nodeStructType("node:crypto::Hash", "rust.node.Hash", "tsonic_rust_node::crypto::Hash"),
    nodeStructType("node:crypto::Hmac", "rust.node.Hmac", "tsonic_rust_node::crypto::Hmac"),
    nodeStructType(
      "node:http::IncomingMessage",
      "rust.node.HttpIncomingMessage",
      "tsonic_rust_node::http::IncomingMessage",
    ),
    nodeStructType(
      "node:http::ServerResponse",
      "rust.node.HttpServerResponse",
      "tsonic_rust_node::http::ServerResponseHandle",
    ),
    nodeStructType(
      "node:http::Server",
      "rust.node.HttpServer",
      "tsonic_rust_node::http::ServerHandle",
    ),
    nodeStructType("node:timers::Timeout", "rust.node.Timeout", "tsonic_rust_node::timers::Timeout"),
    nodeStructType(
      "node:util::TextDecoder",
      "rust.node.TextDecoder",
      "tsonic_rust_node::util::TextDecoder",
    ),
  ]);
  assert.deepEqual(contribution.definition.traitContracts, expectedTraitContracts);
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
  assert.deepEqual(parse.resultCarrier, nodeCarrier(
    "rust.node.UrlObject",
    "tsonic_rust_node::url::LegacyUrlObject",
  ));
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
});

test("provider package closes child-process and text-decoder operations", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const rows = contribution.definition.operations;

  const spawnSync = rows.find((row) => row.exportId === "node:child_process::spawnSync");
  assert.deepEqual(spawnSync, {
    exportId: "node:child_process::spawnSync",
    operationKind: "method",
    target: {
      form: "call",
      path: "node_child_process::spawn_sync_result",
      argModes: ["ref", "ref"],
    },
    resultCarrier: nodeCarrier(
      "rust.node.SpawnSyncResult",
      "tsonic_rust_node::child_process::SpawnSyncResult",
    ),
    parameterCarriers: [
      rustStringTargetType(),
      rustJsArrayTargetType(rustStringTargetType()),
    ],
    isFallible: true,
    errorBoundary: "provider-native",
    errorCarrier: nodeCarrier("rust.node.NodeError", "tsonic_rust_node::NodeError"),
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
      ["stdout", { kind: "type-parameter", name: "T" }],
      ["stderr", { kind: "type-parameter", name: "T" }],
      ["status", {
        kind: "union",
        types: [{ kind: "number" }, { kind: "literal", value: null }],
      }],
    ],
  );
  for (const name of ["stdout", "stderr", "status"]) {
    assert.deepEqual(
      rows
        .filter((row) => row.memberId === `node:child_process::SpawnSyncReturns.${name}`)
        .map((row) => row.operationKind),
      ["property", "property-set"],
      `incomplete SpawnSyncReturns property '${name}'`,
    );
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
    leadingArguments: [{ carrier: rustStringTargetType(), mode: "ref" }],
    elementCarrier: rustJsValueTargetType(),
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
    row.exportId === "node:process::argv" || row.memberId === "node:process.default.argv");
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
  assert.ok(processEnv !== undefined && processEnv.kind === "class");
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
  assert.equal(defaultObject?.name, "NodeProcessModule");
  const defaultExitCode = defaultObject.members.find((member) => member.name === "exitCode");
  assert.equal(defaultExitCode?.readonly, undefined);
  assert.equal(defaultExitCode?.static, true);
  const defaultArgv = defaultObject.members.find((member) => member.name === "argv");
  assert.equal(defaultArgv?.readonly, true);
  const rows = contribution.definition.operations.filter((row) =>
    row.memberId === "node:process.default.exitCode");
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
      ["node:process::hrtime()", "node_process::hrtime_open_number"],
      ["node:process::hrtime(previous)", "node_process::hrtime_since_number"],
    ],
  );
  assert.deepEqual(
    rows.filter((row) => row.memberId === "node:process.default.hrtime").map((row) => [row.signatureId, row.target.path]),
    [
      ["node:process.default.hrtime()", "node_process::hrtime_open_number"],
      ["node:process.default.hrtime(previous)", "node_process::hrtime_since_number"],
    ],
  );

  const namedMethods = ["availableMemory", "chdir", "constrainedMemory", "memoryUsage", "uptime"];
  for (const name of namedMethods) {
    const named = rows.find((row) => row.exportId === `node:process::${name}`);
    const defaultMember = rows.find((row) => row.memberId === `node:process.default.${name}`);
    assert.ok(named !== undefined, `missing named process row '${name}'`);
    assert.ok(defaultMember !== undefined, `missing default process row '${name}'`);
    assert.deepEqual(defaultMember.target, named.target);
    assert.deepEqual(defaultMember.resultCarrier, named.resultCarrier);
  }
  for (const name of ["argv0", "version"]) {
    const named = rows.find((row) => row.exportId === `node:process::${name}`);
    const defaultMember = rows.find((row) => row.memberId === `node:process.default.${name}`);
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
    assert.deepEqual(row?.resultConversion, {
      kind: "semantic-conversion",
      id: "js-number-from-u64",
    });
  }
  assert.deepEqual(
    rows.find((row) => row.exportId === "node:process::memoryUsage")?.resultCarrier,
    nodeCarrier("rust.node.MemoryUsage", "tsonic_rust_node::process::MemoryUsage"),
  );
});

test("provider package closes process stdout and stderr output contracts", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const processModule = contribution.definition.modules.find((module) =>
    module.moduleSpecifier === "node:process");
  assert.ok(processModule !== undefined);
  assert.deepEqual(processModule.imports, [{
    moduleSpecifier: "node:buffer",
    namedImports: [{ exportedName: "Buffer" }],
  }]);
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
      row.memberId === `node:process.default.${name}`);
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
  assert.deepEqual(
    contribution.definition.operations.find((row) => row.exportId === "node:process::stdout")
      ?.resultCarrier,
    nodeCarrier(
      "rust.node.ProcessWriteStream",
      "tsonic_rust_node::process::ProcessWriteStream",
      cloneCopyTraits,
    ),
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
    argModes: ["ref", "ref"],
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
    nodeCarrier("rust.node.Hash", "tsonic_rust_node::crypto::Hash"),
    nodeCarrier("rust.node.Hash", "tsonic_rust_node::crypto::Hash"),
  ]);
  assert.deepEqual(rows.map((row) => row.target.name), ["update_str_owned", "update_buffer_owned"]);
});

test("provider package maps Buffer.from overloads by exact selected signature", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const rows = contribution.definition.operations.filter((row) =>
    row.memberId === "node:buffer::Buffer.from");
  assert.deepEqual(rows.map((row) => row.signatureId), [
    "node:buffer::Buffer.from(string)",
    "node:buffer::Buffer.from(string,encoding)",
    "node:buffer::Buffer.from(numberArray)",
  ]);
  assert.deepEqual(rows.map((row) => row.target.path), [
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
    { kind: "source-primitive", name: "float64" },
    { kind: "source-primitive", name: "float64" },
  ]);
  assert.deepEqual(writeUInt8.map((row) => row.target.receiverMode), ["mut-ref", "mut-ref"]);
  assert.deepEqual(writeUInt8[0].target.trailingArguments, [{ kind: "float64", value: 0 }]);

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
  const { operations, binaryEpilogues } = contribution.definition;

  const statusRead = operations.find((row) =>
    row.memberId === "node:http::ServerResponse.statusCode" && row.operationKind === "property");
  const statusWrite = operations.find((row) =>
    row.memberId === "node:http::ServerResponse.statusCode" && row.operationKind === "property-set");
  assert.deepEqual(statusRead?.target, { form: "receiver-method", name: "status_code" });
  assert.deepEqual(statusWrite?.target, { form: "receiver-method", name: "set_status_code" });
  assert.deepEqual(statusWrite?.parameterCarriers, [{ kind: "source-primitive", name: "int32" }]);

  const endRows = operations.filter((row) => row.memberId === "node:http::ServerResponse.end");
  assert.deepEqual(endRows.map((row) => row.signatureId), [
    "node:http::ServerResponse.end()",
    "node:http::ServerResponse.end(string)",
    "node:http::ServerResponse.end(buffer)",
  ]);
  assert.deepEqual(endRows.map((row) => row.target.name), ["end_empty", "end_string", "end_buffer"]);

  const listenRows = operations.filter((row) => row.memberId === "node:http::Server.listen");
  assert.deepEqual(listenRows.map((row) => row.target.name), ["listen_default_host", "listen"]);
  assert.equal(listenRows.every((row) => row.isFallible === true), true);
  assert.deepEqual(listenRows.map((row) => row.immediateCallback), [undefined, undefined]);
  const createServer = operations.find((row) => row.exportId === "node:http::createServer");
  assert.equal(createServer?.immediateCallback, undefined);
  assert.deepEqual(
    contribution.definition.types.find((row) => row.exportId === "node:http::ServerResponse")
      ?.targetCarrier,
    nodeCarrier(
      "rust.node.HttpServerResponse",
      "tsonic_rust_node::http::ServerResponseHandle",
    ),
  );
  assert.deepEqual(binaryEpilogues, [
    {
      id: "node-event-loop",
      path: "tsonic_rust_node::run_event_loop",
      requiredCrate: "tsonic_rust_node",
      isFallible: true,
      errorBoundary: "source-program",
    },
    {
      id: "node-process-exit-code",
      path: "tsonic_rust_node::process::apply_exit_code",
      requiredCrate: "tsonic_rust_node",
    },
  ]);
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
  assert.deepEqual(
    rows.map((row) => row.resultCarrier),
    [
      nodeCarrier("rust.node.Timeout", "tsonic_rust_node::timers::Timeout"),
      nodeCarrier("rust.node.Timeout", "tsonic_rust_node::timers::Timeout"),
    ],
  );
});
