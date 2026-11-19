import assert from "node:assert/strict";
import test from "node:test";
import { compileRust, rustSourceDiagnostics, createRustSession } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

function compile(source) {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "lib", crateName: "readable_owner_source" } },
    files: { "index.ts": source },
  });
  assert.equal(result.diagnostics.length, 0, result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
  return result.artifacts.filter(row => row.path.endsWith(".rs")).map(row => row.text).join("\n");
}

test("authored readable factories receive a single real task owner without extra source arguments", () => {
  const source = compile(`import { Readable } from "node:stream";
    import { Buffer } from "node:buffer";
    import { createReadStream } from "node:fs";
    import { createGzip } from "node:zlib";
    export function open(): void {
      const input = Readable.from([Buffer.from("native bytes")]);
      input.resume();
      createReadStream("fixture.txt").resume();
      createGzip().resume();
    }`);
  assert.equal([...source.matchAll(/pub static dispatch_root_\d+:.*RuntimeTasks</gu)].length, 1);
  assert.match(source, /Readable::from_source\([^;]*\.handle\(\)/u);
  assert.match(source, /create_read_stream\([^;]*\.handle\(\)/u);
  assert.match(source, /create_gzip\([^;]*\.handle\(\)/u);
});

test("named and default stdin access share typed resource and task roots", () => {
  const source = compile(`import process, { stdin } from "node:process";
    export function start(): void { stdin.resume(); process.stdin.resume(); }`);
  for (const owner of ["StdinResources", "BackgroundTasks", "RuntimeTasks"]) {
    assert.equal([...source.matchAll(new RegExp(`pub static dispatch_root_\\d+:.*${owner}<`, "gu"))].length, 1, owner);
  }
  assert.equal([...source.matchAll(/tsonic_rust_node::process::stdin(?:[^(\n]*)\([^;]*\.handle\(\)/gu)].length, 2);
  assert.doesNotMatch(source, /std::io::stdin\(\)\.read_line/u);
});

test("source validation retains required input, exact callbacks and argument rejection", () => {
  for (const body of [
    `createInterface({});`,
    `const input = Readable.from([Buffer.from("line\\n")]); const reader = createInterface({ input }); reader.question("", 42);`,
    `Readable.from([42]);`,
    `const input = Readable.from([Buffer.from("line\\n")]); input.once("end", (value: number): void => {});`,
  ]) {
    const harness = createRustSession({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
      files: { "index.ts": `import { createInterface } from "node:readline";
        import { Readable } from "node:stream"; import { Buffer } from "node:buffer";
        export function invalid(): void { ${body} }` },
    });
    const diagnostics = rustSourceDiagnostics(harness);
    assert.equal(diagnostics.length > 0, true, body);
  }
});

test("readline optional native fields use typed absence without manufacturing a required input default", () => {
  for (const options of ["{ input }", "options"]) {
    const source = compile(`import { createInterface } from "node:readline";
      import { Readable } from "node:stream"; import { Buffer } from "node:buffer";
      export function start(): void {
        const input = Readable.from([Buffer.from("answer\\n")]);
        const options = { input };
        const reader = createInterface(${options});
        reader.question("", answer => { if (answer !== "answer") throw new Error("wrong native answer"); });
      }`);
    assert.match(source, /SourceInterfaceOptions[^{]*\{/u);
    for (const field of ["output", "terminal", "prompt"]) {
      assert.match(source, new RegExp(`${field}: None`, "u"), field);
    }
    assert.doesNotMatch(source, /Default::default\(\)|Readable::default\(\)/u);
    assert.equal([...source.matchAll(/pub static dispatch_root_\d+:.*RuntimeTasks</gu)].length, 1);
    assert.doesNotMatch(source, /BackgroundTasks</u);
  }
});
