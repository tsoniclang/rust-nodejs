import assert from "node:assert/strict";
import test from "node:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { rustBorrowedStrTargetType, rustOptionTargetType } from "@tsonic/target-rust/provider";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

test("HTTP metadata operation carriers expose their exact native receiver borrows", () => {
  const [contribution] = createTsonicPlugin().createTargetContributions({});
  const rows = contribution.definition.operations;
  const incoming = contribution.definition.modules.find(module => module.moduleSpecifier === "node:http")
    .exports.find(entry => entry.name === "IncomingMessage");
  const headers = contribution.definition.types.find(type => type.exportId === "node:http::IncomingHttpHeaders").targetCarrier;
  for (const name of ["method", "url", "httpVersion", "headers", "headersDistinct", "statusMessage"]) {
    const declaration = incoming.members.find(member => member.name === name);
    assert.equal(declaration.readonly, true, name);
    const row = rows.find(row => row.exportId === incoming.id && row.memberId === `${incoming.id}.${name}`);
    const borrowedString = rustBorrowedStrTargetType();
    if (name === "method" || name === "url" || name === "statusMessage") {
      assert.deepEqual(row.resultCarrier, rustOptionTargetType(borrowedString), name);
    } else if (name === "httpVersion") {
      assert.deepEqual(row.resultCarrier, borrowedString, name);
    } else {
      assert.equal(row.resultCarrier.kind, "reference", name);
      assert.equal(row.resultCarrier.mutable, false, name);
      assert.deepEqual(row.resultCarrier.referent, headers, name);
    }
  }
});

test("authored incoming metadata views compile and execute without read allocations or lost owned results", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "lib", crateName: "borrowed_http_metadata" } },
    files: { "index.ts": `import type { IncomingMessage } from "node:http";
      export function inspect(request: IncomingMessage): boolean {
        const method = request.method;
        const url = request.url;
        const phrase = request.statusMessage;
        const version = request.httpVersion;
        return method === "POST" && url === "/native-resource" &&
          phrase === undefined && version === "1.1";
      }
      export function compare(request: IncomingMessage, expected: string, optional: string | undefined): boolean {
        return request.method === optional && optional === request.method && request.method === expected &&
          expected === request.method;
      }
      export function ordered(request: IncomingMessage, expected: string): boolean {
        return request.httpVersion >= expected && request.httpVersion <= expected;
      }
      export function version(request: IncomingMessage): string { return request.httpVersion; }
      export function method(request: IncomingMessage): string | undefined { return request.method; }
      export function retained(request: IncomingMessage): () => string { return () => request.httpVersion; }
      export function length(request: IncomingMessage): number { return request.httpVersion.length; }
      export function contains(request: IncomingMessage): boolean { return request.httpVersion.includes(request.httpVersion); }
      export function categories(request: IncomingMessage): boolean {
        return typeof request.httpVersion === "string" && typeof request.method === "string";
      }
      export function text(request: IncomingMessage): string { return String(request.httpVersion); }
      export function optionaltext(request: IncomingMessage): string { return String(request.method); }
      export function numeric(request: IncomingMessage): number { return Number(request.httpVersion); }
      export function optionalnumeric(request: IncomingMessage): number { return Number(request.statusMessage); }
      export function pad(request: IncomingMessage): string { return request.httpVersion.padStart(5, request.httpVersion); }` },
  });
  assert.equal(result.diagnostics.length, 0, result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
  const source = result.artifacts.filter(artifact => artifact.path.endsWith(".rs")).map(artifact => artifact.text).join("\n");
  assert.match(source, /\.method\(\)/u);
  assert.match(source, /\.http_version\(\)/u);
  const contains = source.slice(source.indexOf("pub fn contains("), source.indexOf("pub fn categories("));
  assert.doesNotMatch(contains, /String::from|\.clone\(|\.to_owned\(/u);
  const root = writeGeneratedProject("borrowed-http-metadata", result.artifacts);
  const allocationSupport = fileURLToPath(new URL("../../../rust/tests/support/allocation_counts.rs", import.meta.url));
  mkdirSync(join(root, "tests"), { recursive: true });
  const nativeTestPath = join(root, "tests", "http_metadata.rs");
  writeFileSync(nativeTestPath, `
#[path = ${JSON.stringify(allocationSupport)}]
mod allocation_counts;

use borrowed_http_metadata::index;
use tsonic_rust_node::http::IncomingMessage;
use tsonic_rust_runtime::TsonicError;

#[test]
fn generated_metadata_reads_and_borrowed_string_arguments_do_not_allocate() {
    let message = IncomingMessage::<TsonicError>::new("POST", "/native-resource", Vec::new());
    let cost = allocation_counts::measure(|| {
        for _ in 0..10_000 {
            assert!(std::hint::black_box(index::inspect(message.clone())));
            assert!(std::hint::black_box(index::contains(message.clone())));
            assert_eq!(std::hint::black_box(index::length(message.clone())), 3.0);
            assert!(std::hint::black_box(index::categories(message.clone())));
            assert_eq!(std::hint::black_box(index::numeric(message.clone())), 1.1);
            assert_eq!(std::hint::black_box(index::optionalnumeric(message.clone())), 0.0);
        }
    });
    assert_eq!(cost, (0, 0));
    assert!(index::compare(message.clone(), "POST", Some("POST".to_owned())));
    assert!(!index::compare(message.clone(), "POST", None));
    assert!(index::ordered(message.clone(), "1.1".to_owned()));
    assert!(!index::ordered(message.clone(), "0.9".to_owned()));
}

#[test]
fn generated_owned_results_and_retained_callbacks_outlive_their_message_argument() {
    let message = IncomingMessage::<TsonicError>::new("POST", "/native-resource", Vec::new());
    let mut version = String::new();
    let version_cost = allocation_counts::measure(|| version = index::version(message.clone()));
    assert_eq!(version_cost, (1, 3));
    let mut method = None;
    let method_cost = allocation_counts::measure(|| method = index::method(message.clone()));
    assert_eq!(method_cost, (1, 4));
    let callback = index::retained(message.clone());
    drop(message);
    assert_eq!(version, "1.1");
    assert_eq!(method.as_deref(), Some("POST"));
    assert_eq!(callback.call(()).unwrap(), "1.1");
    assert_eq!(callback.call(()).unwrap(), "1.1");
    let next = IncomingMessage::<TsonicError>::new("POST", "/native-resource", Vec::new());
    assert_eq!(index::text(next.clone()), "1.1");
    assert_eq!(index::optionaltext(next.clone()), "POST");
    assert_eq!(index::pad(next).unwrap(), "1.1.1");
}
`);
  runCargo(root, ["generate-lockfile", "--offline"]);
  const formatted = spawnSync("rustfmt", [nativeTestPath], { encoding: "utf8", timeout: 30_000 });
  assert.equal(formatted.status, 0, (formatted.stderr ?? "").slice(0, 4096));
  runCargo(root, ["fmt", "--all", "--check"]);
  runCargo(root, ["check", "--all-targets", "--locked", "--offline"]);
  runCargo(root, ["clippy", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"]);
  runCargo(root, ["test", "--locked", "--offline"]);
});
