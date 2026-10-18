import assert from "node:assert/strict";
import test from "node:test";
import { appendFileSync } from "node:fs";
import { join } from "node:path";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, validateGeneratedProject, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeTlsCompletionSource, nativeTlsFailureSource } from "../../../../tsonic/test/fixtures/native-tls-contexts.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

function compile(source, outputType = "bin") {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType, crateName: "native_tls_context" } },
    files: { "index.ts": source },
  });
  assert.equal(result.diagnostics.length, 0, result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
  return result;
}

test("generated native Node driver consumes exact TLS and background roots", { timeout: 300_000 }, () => {
  const result = compile(`${nativeTlsCompletionSource}\nmain();\n`);
  const output = validateGeneratedProject("native-tls-context", result.artifacts, { run: true });
  assert.match(output.stdout, /native TLS completion/u);
});

test("original source TLS failure preserves later native listening callbacks", { timeout: 300_000 }, () => {
  const result = compile(nativeTlsFailureSource, "lib");
  const roots = result.artifacts.filter(row => row.path.endsWith(".rs") && /pub static dispatch_root_/u.test(row.text));
  assert.equal(roots.length, 1, "one exact component root module");
  const names = [...roots[0].text.matchAll(/pub static (dispatch_root_\d+):/gu)].map(match => match[1]);
  assert.equal(names.length, 2, "only the demanded TLS and background roots");
  const tail = "tsonic_rust_runtime::dispatch::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new()";
  const contexts = names.reduceRight((rest, _name, index) => `tsonic_rust_runtime::dispatch::prepend(root_${index}, ${rest})`, tail);
  const entry = names.reduceRight((rest, name, index) => `super::${name}.with(|root_${index}| ${rest})`,
    `tsonic_rust_node::run_entry_with_contexts(|| {
      crate::initialize(); expected = Some(crate::schedule().expect("source registration"));
    }, ${contexts})`);
  const freshContexts = names.reduceRight((rest, _name, index) => `tsonic_rust_runtime::dispatch::prepend(root_${index}, ${rest})`,
    `tsonic_rust_runtime::dispatch::prepend(&fresh, ${tail})`);
  const freshEntry = names.reduceRight((rest, name, index) => `super::${name}.with(|root_${index}| ${rest})`,
    `tsonic_rust_node::run_entry_with_contexts(|| {
      fresh.enqueue(move || { observed.set(observed.get() + 1); Ok(()) }).unwrap();
    }, ${freshContexts})`);
  const noRetainedWork = names.map(name => `super::${name}.with(|root| assert!(!root.has_work()));`).join("\n");
  const directory = writeGeneratedProject("native-tls-original-error", result.artifacts);
  appendFileSync(join(directory, roots[0].path), `
#[cfg(test)]
mod retained_source_failure {
    use tsonic_rust_runtime::ErrorObject;
    use tsonic_rust_runtime::dispatch::DispatchContexts;
    #[test]
    fn original_error_stops_and_preserves_native_tls_listeners() {
        let mut expected = None;
        let returned = ${entry}.expect_err("original source callback must fail");
        let expected = expected.expect("the source registration completed before dispatch");
        assert_eq!(returned.source_error().error_identity_key(), expected.error_identity_key());
        assert_eq!(returned.source_error().error_message(), expected.error_message());
        assert_eq!(crate::count(), 0.0);
        ${noRetainedWork}
        let fresh = tsonic_rust_node::runtime_tasks::RuntimeTasks::<tsonic_rust_runtime::TsonicError>::new();
        let delivered = std::rc::Rc::new(std::cell::Cell::new(0));
        let observed = std::rc::Rc::clone(&delivered);
        ${freshEntry}.unwrap();
        assert_eq!(crate::count(), 0.0);
        assert_eq!(delivered.get(), 1);
        assert!(!fresh.has_pending_work());
        ${noRetainedWork}
        assert!(tsonic_rust_js::event_loop::dispatch_owner().is_err());
    }
}
`);
  for (const args of [["generate-lockfile", "--offline"], ["fmt", "--all"], ["fmt", "--all", "--check"],
    ["check", "--all-targets", "--locked", "--offline"], ["clippy", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"],
    ["test", "--locked", "--offline"]]) runCargo(directory, args);
});
