import assert from "node:assert/strict";
import test from "node:test";
import { appendFileSync } from "node:fs";
import { join } from "node:path";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, validateGeneratedProject, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

const synchronousCases = Object.freeze([
  Object.freeze(["original-error", `
import { createGzip } from "node:zlib";

export function main(): void {
  const original = new Error("original finish failure");
  const stream = createGzip();
  stream.once("finish", (): void => { throw original; });
  let retained = false;
  try { stream.end(); }
  catch (failure) {
    if (failure instanceof Error) {
      retained = failure === original && failure.message === "original finish failure";
    }
  }
  stream.destroy();
  if (!retained || !stream.writableFinished || !stream.destroyed) {
    throw new Error("finish callback did not preserve the original Error identity");
  }
}
`]),
  Object.freeze(["closed-payload", `
import { createGzip } from "node:zlib";
import type { int64 } from "@tsonic/core/types.js";

class ClosedFailure {
  readonly value: int64;
  constructor(value: int64) { this.value = value; }
}

export function main(): void {
  const exact: int64 = 9007199254740993n;
  const original = new ClosedFailure(exact);
  const stream = createGzip();
  stream.once("finish", (): void => { throw original; });
  let retained = false;
  try { stream.end(); }
  catch (failure) {
    if (failure instanceof ClosedFailure) retained = failure.value === exact;
  }
  stream.destroy();
  if (!retained || !stream.writableFinished || !stream.destroyed) {
    throw new Error("finish callback did not preserve its exact closed non-Error payload");
  }
}
`]),
]);

for (const [name, source] of synchronousCases) {
  test(`synchronous native Gzip finish callback preserves ${name}`, { timeout: 300_000 }, () => {
    const { result } = compileRust({
      surfaces: ["js"],
      capabilities: [createTsonicPlugin()],
      target: { id: "rust", options: { outputType: "bin" } },
      files: { "index.ts": source },
    });
    assert.equal(result.diagnostics.length, 0,
      result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
    const execution = validateGeneratedProject(`native-stream-callback-${name}`, result.artifacts, { run: true });
    assert.equal(execution.status, 0);
  });
}

test("deferred native file finish callback preserves the original Error at the dispatch root", { timeout: 300_000 }, () => {
  const { result } = compileRust({
    surfaces: ["js"],
    capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "lib", crateName: "native_stream_callback_domain" } },
    files: { "index.ts": `
import { createWriteStream } from "node:fs";

export function schedule(): Error {
  const original = new Error("original deferred finish failure");
  const stream = createWriteStream("native-stream-callback-output.txt");
  stream.once("finish", (): void => { throw original; });
  stream.end("native output");
  return original;
}
` },
  });
  assert.equal(result.diagnostics.length, 0,
    result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
  const roots = result.artifacts.filter(row => row.path.endsWith(".rs") && /pub static __tsonic_dispatch_/u.test(row.text));
  assert.equal(roots.length, 1, "one exact component root module");
  const names = [...roots[0].text.matchAll(/pub static (__tsonic_dispatch_\d+):/gu)].map(match => match[1]);
  assert.equal(names.length, 1, "only the demanded background root");
  const run = `super::${names[0]}.with(|root| tsonic_rust_node::run_with_contexts(tsonic_rust_runtime::dispatch::prepend(root, tsonic_rust_runtime::dispatch::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new())))`;
  const directory = writeGeneratedProject("native-stream-callback-deferred", result.artifacts);
  appendFileSync(join(directory, roots[0].path), `
#[cfg(test)]
mod source_callback_failures {
    use tsonic_rust_runtime::ErrorObject;

    #[test]
    fn deferred_finish_retains_original_source_error() {
        crate::initialize();
        let expected = crate::schedule().expect("file stream callback registration");
        let failure = ${run}.expect_err("deferred callback must fail");
        let output = std::fs::read("native-stream-callback-output.txt").unwrap();
        std::fs::remove_file("native-stream-callback-output.txt").unwrap();
        assert_eq!(output, b"native output");
        assert_eq!(failure.source_error().error_identity_key(), expected.error_identity_key());
        assert_eq!(failure.source_error().error_message(), expected.error_message());
    }
}
`);
  runCargo(directory, ["generate-lockfile", "--offline"]);
  runCargo(directory, ["fmt", "--all"]);
  runCargo(directory, ["fmt", "--all", "--check"]);
  runCargo(directory, ["check", "--all-targets", "--locked", "--offline"]);
  runCargo(directory, ["clippy", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"]);
  runCargo(directory, ["test", "--locked", "--offline", "--", "--test-threads=1"]);
});
