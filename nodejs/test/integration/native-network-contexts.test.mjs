import assert from "node:assert/strict";
import test from "node:test";
import { appendFileSync } from "node:fs";
import { join } from "node:path";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, validateGeneratedProject, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeNetworkCompletionSource, nativeNetworkFailureSource, nativeNetworkNoDemandSource } from "../../../../tsonic/test/fixtures/native-network-contexts.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

function compile(source, outputType = "bin") {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType, crateName: "native_network_context" } },
    files: { "index.ts": source },
  });
  assert.equal(result.diagnostics.length, 0, result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
  return result;
}

test("generated native Node driver consumes the exact native network root", { timeout: 300_000 }, () => {
  const result = compile(nativeNetworkCompletionSource);
  const output = validateGeneratedProject("native-network-context", result.artifacts, { run: true });
  assert.match(output.stdout, /native network completion/u);
});

test("native network queries require no callback roots", { timeout: 300_000 }, () => {
  const result = compile(nativeNetworkNoDemandSource);
  assert.equal(result.artifacts.some(row => /pub static __tsonic_dispatch_/u.test(row.text)), false,
    "queries must not manufacture retained callback roots");
  validateGeneratedProject("native-network-no-demand", result.artifacts, { run: true });
});

test("original source network failure preserves later native listening callbacks", { timeout: 300_000 }, () => {
  const result = compile(nativeNetworkFailureSource, "lib");
  const roots = result.artifacts.filter(row => row.path.endsWith(".rs") && /pub static __tsonic_dispatch_/u.test(row.text));
  assert.equal(roots.length, 1, "one exact component root module");
  const names = [...roots[0].text.matchAll(/pub static (__tsonic_dispatch_\d+):/gu)].map(match => match[1]);
  assert.equal(names.length, 1, "only the demanded native network root");
  const directory = writeGeneratedProject("native-network-original-error", result.artifacts);
  appendFileSync(join(directory, roots[0].path), `
#[cfg(test)]
mod retained_source_failure {
    use tsonic_rust_runtime::ErrorObject;
    #[test]
    fn original_error_stops_and_preserves_pending_native_listeners() {
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
