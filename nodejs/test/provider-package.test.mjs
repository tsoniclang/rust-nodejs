import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../dist/index.js";

const expectedModules = [
  "node:assert",
  "node:path",
  "node:os",
  "node:fs",
  "node:fs/promises",
  "node:process",
  "node:buffer",
  "node:url",
  "node:crypto",
  "node:util",
  "node:http",
  "node:timers",
];

test("provider package declares the expected node module specifiers", () => {
  const plugin = createTsonicPlugin();
  const specifiers = plugin.moduleOwnership.map((ownership) => ownership.specifierPrefix);
  for (const moduleSpecifier of expectedModules) {
    assert.ok(specifiers.includes(moduleSpecifier), `missing module '${moduleSpecifier}'`);
  }
  assert.equal(specifiers.length, expectedModules.length);
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
    ["node:fs::Stats", "rust.node.Stats"],
    ["node:process::ProcessEnv", "rust.node.ProcessEnv"],
    ["node:buffer::Buffer", "rust.node.Buffer"],
    ["node:url::URL", "rust.node.Url"],
    ["node:url::UrlObject", "rust.node.UrlObject"],
    ["node:url::URLSearchParams", "rust.node.UrlSearchParams"],
    ["node:crypto::Hash", "rust.node.Hash"],
    ["node:crypto::Hmac", "rust.node.Hmac"],
    ["node:http::IncomingMessage", "rust.node.HttpIncomingMessage"],
    ["node:http::ServerResponse", "rust.node.HttpServerResponse"],
    ["node:http::Server", "rust.node.HttpServer"],
    ["node:timers::Timeout", "rust.node.Timeout"],
  ].map(([exportId, id]) => ({
    exportId,
    targetCarrier: { kind: "target-named", id },
  })));
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
  assert.ok(fs.exports.some((entry) => entry.id === "node:fs::mkdtempSync"));
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

test("provider package maps HTTP server mutation and lifecycle contracts exactly", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const { operations, binaryEpilogues, carrierPaths } = contribution.definition;

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
  assert.deepEqual(carrierPaths["rust.node.HttpServerResponse"],
    "tsonic_rust_node::http::ServerResponseHandle");
  assert.deepEqual(binaryEpilogues, [{
    id: "node-event-loop",
    path: "tsonic_rust_node::run_event_loop",
    requiredCrate: "tsonic_rust_node",
    isFallible: true,
  }]);
});

test("provider package maps timers to the shared Node event loop", () => {
  const plugin = createTsonicPlugin();
  const [contribution] = plugin.createTargetContributions({});
  const row = contribution.definition.operations.find((candidate) =>
    candidate.exportId === "node:timers::setInterval");
  assert.ok(row !== undefined);
  assert.equal(row.operationKind, "method");
  assert.deepEqual(row.target, { form: "call", path: "node_timers::set_interval_callable" });
  assert.equal(row.immediateCallback, undefined);
  assert.deepEqual(row.resultCarrier, { kind: "target-named", id: "rust.node.Timeout" });
});
