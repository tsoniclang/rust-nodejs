import { assertNoTargetDiagnostics } from "../../../../tsonic/test/scripts/diagnostic-assertions.mjs";
import assert from "node:assert/strict";
import test from "node:test";
import { acmeTestingPackage, artifactText, compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { validateGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

test("Node optional arguments preserve omitted and explicit absence at native calls", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    packages: [acmeTestingPackage()], target: { id: "rust", options: { outputType: "bin" } }, files: { "index.ts": `
    import { check } from "@acme/testing";
    import { createServer } from "node:http";
    import { Readable } from "node:stream";
    import { Buffer } from "node:buffer";
    export function run(): boolean {
      const server = createServer();
      const explicit = createServer(undefined);
      const source = Readable.from([Buffer.from("data")]);
      const chunk = source.read();
      source.destroy();
      source.destroy(undefined);
      return !server.listening && !explicit.listening && source.destroyed && chunk !== undefined && chunk.length === 4;
    }
    export function main(): void { check(run()); }
  ` } });
  assertNoTargetDiagnostics(result.diagnostics);
  const output = artifactText(result, "src/index.rs");
  assert.equal(output.match(/destroy_chain\(Option::<tsonic_rust_runtime::RetainedError>::None\)/gu)?.length, 2);
  validateGeneratedProject("native-optional-arguments", result.artifacts, { run: true });
});
