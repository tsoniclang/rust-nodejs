import assert from "node:assert/strict";
import test from "node:test";
import { appendFileSync } from "node:fs";
import { join } from "node:path";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, validateGeneratedProject, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeRuntimeCompletionSource, nativeRuntimeOriginalErrorSource } from "../../../../tsonic/test/fixtures/native-runtime-contexts.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

function compile(source, outputType = "bin") {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType, crateName: "native_runtime_context" } },
    files: { "index.ts": source },
  });
  assert.equal(result.diagnostics.length, 0, result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
  return result;
}

test("generated Node driver selects source-thread readline tasks without worker scheduling", { timeout: 300_000 }, () => {
  const result = compile(nativeRuntimeCompletionSource);
  const output = validateGeneratedProject("native-runtime-context", result.artifacts, { run: true });
  assert.match(output.stdout, /native runtime completion/u);
});

test("generated readline callback retains its original Error identity through native context grouping", { timeout: 300_000 }, () => {
  const result = compile(nativeRuntimeOriginalErrorSource, "lib");
  const roots = result.artifacts.filter(row => row.path.endsWith(".rs") && /pub static __tsonic_dispatch_/u.test(row.text));
  assert.equal(roots.length, 1, "one exact component-owned root module");
  const names = [...roots[0].text.matchAll(/pub static (__tsonic_dispatch_\d+):/gu)].map(match => match[1]);
  assert.equal(names.length, 2, "independent background and buffered-task roots");
  const directory = writeGeneratedProject("native-runtime-original-error", result.artifacts);
  appendFileSync(join(directory, roots[0].path), `
#[cfg(test)]
mod retained_source_failure {
    use tsonic_rust_runtime::ErrorObject;
    #[test]
    fn original_error_survives_source_thread_dispatch() {
        crate::initialize();
        let expected = crate::schedule().expect("callback registration");
        let result = super::${names[0]}.with(|background| super::${names[1]}.with(|tasks| {
            tsonic_rust_node::run_with_contexts(tsonic_rust_runtime::dispatch::prepend(background,
                tsonic_rust_runtime::dispatch::prepend(tasks, tsonic_rust_runtime::dispatch::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new())))
        }));
        let returned = result.expect_err("source callback must fail");
        assert_eq!(returned.source_error().error_identity_key(), expected.error_identity_key());
        assert_eq!(returned.source_error().error_message(), expected.error_message());
    }
}
`);
  for (const args of [["generate-lockfile", "--offline"], ["fmt", "--all"], ["fmt", "--all", "--check"],
    ["check", "--all-targets", "--locked", "--offline"], ["clippy", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"],
    ["test", "--locked", "--offline"]]) runCargo(directory, args);
});
