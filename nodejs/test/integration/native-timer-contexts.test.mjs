import assert from "node:assert/strict";
import test from "node:test";
import { appendFileSync } from "node:fs";
import { join } from "node:path";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, validateGeneratedProject, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeTimerCompletionSource, nativeTimerOriginalErrorSource, nativeJsTimerCompletionSource } from "../../../../tsonic/test/fixtures/native-timer-contexts.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

function compile(source, outputType = "bin", node = true) {
  const { result } = compileRust({ surfaces: ["js"], capabilities: node ? [createTsonicPlugin()] : [],
    target: { id: "rust", options: { outputType, crateName: "native_timer_context" } },
    files: { "index.ts": source },
  });
  assert.equal(result.diagnostics.length, 0, result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
  return result;
}

test("generated Node driver drains independent Node and JS timer roots", { timeout: 300_000 }, () => {
  const result = compile(nativeTimerCompletionSource);
  const output = validateGeneratedProject("native-timer-context", result.artifacts, { run: true });
  assert.match(output.stdout, /native timer completion/u);
  const entry = result.artifacts.find(row => row.path === "src/main.rs");
  assert.equal(entry !== undefined, true, "native binary entry");
  assert.equal(entry.text.includes("tsonic_rust_js::event_loop::run_with_contexts"), false, "one selected native driver");
  assert.equal(entry.text.includes("tsonic_rust_node::run_with_contexts"), true, "Node driver owns both selected timer families");
});

test("generated JS-only driver selects the same native typed timer owner", { timeout: 300_000 }, () => {
  const result = compile(nativeJsTimerCompletionSource, "bin", false);
  const output = validateGeneratedProject("native-js-timer-context", result.artifacts, { run: true });
  assert.match(output.stdout, /native timer completion/u);
  assert.equal(result.artifacts.some(row => row.text.includes("tsonic_rust_node")), false, "no unnecessary Node dependency");
});

test("original timer error stops dispatch without consuming another selected root", { timeout: 300_000 }, () => {
  const result = compile(nativeTimerOriginalErrorSource, "lib");
  const roots = result.artifacts.filter(row => row.path.endsWith(".rs") && /pub static __tsonic_dispatch_/u.test(row.text));
  assert.equal(roots.length, 1, "one exact component root module");
  const names = [...roots[0].text.matchAll(/pub static (__tsonic_dispatch_\d+):/gu)].map(match => match[1]);
  assert.equal(names.length, 2, "only demanded Node and JS timer roots");
  const group = names.reduceRight((tail, _name, index) => `tsonic_rust_runtime::dispatch::prepend(root_${index}, ${tail})`,
    "tsonic_rust_runtime::dispatch::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new()");
  const run = names.reduceRight((tail, name, index) => `super::${name}.with(|root_${index}| ${tail})`,
    `tsonic_rust_node::run_with_contexts(${group})`);
  const directory = writeGeneratedProject("native-timer-original-error", result.artifacts);
  appendFileSync(join(directory, roots[0].path), `
#[cfg(test)]
mod retained_source_failure {
    use tsonic_rust_runtime::ErrorObject;
    #[test]
    fn original_error_stops_and_preserves_pending_source_work() {
        crate::initialize();
        let expected = crate::schedule().expect("source registration");
        let returned = ${run}.expect_err("original source timer must fail");
        assert_eq!(returned.source_error().error_identity_key(), expected.error_identity_key());
        assert_eq!(returned.source_error().error_message(), expected.error_message());
        assert_eq!(crate::count(), 0.0);
        assert!(${run}.is_ok());
        assert_eq!(crate::count(), 1.0);
    }
}
`);
  for (const args of [["generate-lockfile", "--offline"], ["fmt", "--all"], ["fmt", "--all", "--check"],
    ["check", "--all-targets", "--locked", "--offline"], ["clippy", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"],
    ["test", "--locked", "--offline"]]) runCargo(directory, args);
});
