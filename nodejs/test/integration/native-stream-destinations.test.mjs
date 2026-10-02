import assert from "node:assert/strict";
import test from "node:test";
import { acmeTestingPackage, compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { validateGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

test("native pipe preserves writable, transform and HTTP destination types and identity", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    packages: [acmeTestingPackage()], target: { id: "rust", options: { outputType: "bin" } }, files: { "index.ts": `
    import { check } from "@acme/testing";
    import { Readable, Writable, Transform } from "node:stream";
    import { Buffer } from "node:buffer";
    import type { ServerResponse } from "node:http";
    import { createGzip } from "node:zlib";
    export function pipeWritable(source: Readable, destination: Writable): Writable {
      return source.pipe(destination);
    }
    export function pipeTransform(source: Readable, destination: Transform): Transform {
      return source.pipe(destination);
    }
    export function pipeResponse(source: Readable, destination: ServerResponse): ServerResponse {
      return source.pipe(destination);
    }
    export function run(): boolean {
      const source = Readable.from([Buffer.from("stream")]);
      const destination = createGzip();
      return source.pipe(destination) === destination;
    }
    export function main(): void { check(run()); }
  ` } });
  assert.deepEqual(result.diagnostics, []);
  validateGeneratedProject("native-stream-destinations", result.artifacts, { run: true });
});

test("native pipe rejects a non-writable destination before publication", () => {
  assert.throws(() => compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()], files: { "index.ts": `
    import { Readable } from "node:stream";
    export function pipe(source: Readable, destination: Readable): Readable {
      return source.pipe(destination);
    }
  ` } }), /TypeScript diagnostics:[\s\S]*Type 'Readable' is missing[\s\S]*from type 'Writable'/);
});
