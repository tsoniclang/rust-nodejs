import assert from "node:assert/strict";
import test from "node:test";
import { appendFileSync } from "node:fs";
import { join } from "node:path";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, validateGeneratedProject, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeWatcherCompletionSource, nativeWatcherFailureSource, nativeWatcherNoDemandSource } from "../../../../tsonic/test/fixtures/native-watcher-contexts.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

function compile(source, outputType = "bin") {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType, crateName: "native_watcher_context" } },
    files: { "index.ts": source },
  });
  assert.equal(result.diagnostics.length, 0, result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
  return result;
}

test("generated native Node driver consumes the exact native watcher root", { timeout: 300_000 }, () => {
  const result = compile(nativeWatcherCompletionSource);
  const output = validateGeneratedProject("native-watcher-context", result.artifacts, { run: true });
  assert.match(output.stdout, /native watcher completion/u);
});

test("native filesystem queries require no watcher roots", { timeout: 300_000 }, () => {
  const result = compile(nativeWatcherNoDemandSource);
  assert.equal(result.artifacts.some(row => /pub static __tsonic_dispatch_/u.test(row.text)), false,
    "filesystem queries must not manufacture retained callback roots");
  validateGeneratedProject("native-watcher-no-demand", result.artifacts, { run: true });
});

test("original source watcher failure preserves reentrant native stat work", { timeout: 300_000 }, () => {
  const result = compile(nativeWatcherFailureSource, "lib");
  const roots = result.artifacts.filter(row => row.path.endsWith(".rs") && /pub static __tsonic_dispatch_/u.test(row.text));
  assert.equal(roots.length, 1, "one exact component root module");
  const names = [...roots[0].text.matchAll(/pub static (__tsonic_dispatch_\d+):/gu)].map(match => match[1]);
  assert.equal(names.length, 1, "only the demanded native watcher root");
  const directory = writeGeneratedProject("native-watcher-original-error", result.artifacts);
  appendFileSync(join(directory, roots[0].path), `
#[cfg(test)]
mod retained_source_failure {
    use tsonic_rust_runtime::ErrorObject;
    #[test]
    fn original_error_keeps_reentrant_native_stat_work() {
        crate::initialize();
        let expected = crate::schedule().expect("source registration");
        let run = || super::${names[0]}.with(|root| tsonic_rust_node::run_with_contexts(
            tsonic_rust_runtime::dispatch::prepend(root,
                tsonic_rust_runtime::dispatch::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new())));
        let returned = run().expect_err("original source callback must fail");
        assert_eq!(returned.source_error().error_identity_key(), expected.error_identity_key());
        assert_eq!(returned.source_error().error_message(), expected.error_message());
        assert_eq!(crate::count(), 0.0);
        assert!(run().is_ok());
        assert_eq!(crate::count(), 1.0);
    }
}
`);
  for (const args of [["generate-lockfile", "--offline"], ["fmt", "--all"], ["fmt", "--all", "--check"],
    ["check", "--all-targets", "--locked", "--offline"], ["clippy", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"],
    ["test", "--locked", "--offline"]]) runCargo(directory, args);
});
