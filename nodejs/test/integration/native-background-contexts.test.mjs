import assert from "node:assert/strict";
import test from "node:test";
import { appendFileSync } from "node:fs";
import { join } from "node:path";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, validateGeneratedProject, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";
import { nativeBackgroundCompletionSource, nativeBackgroundAsyncSource,
  nativeBackgroundOriginalErrorSource, nativeBackgroundNoDemandSource,
  nativeBackgroundPackageFiles, nativeBackgroundPackageGraph } from "../../../../tsonic/test/fixtures/native-background-contexts.mjs";

function compile(source, outputType = "bin") {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType, crateName: "native_background_context" } },
    files: { "index.ts": source },
  });
  assert.equal(result.diagnostics.length, 0,
    result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
  return result;
}

test("generated native Node driver consumes the demanded compression context", { timeout: 300_000 }, () => {
  const result = compile(nativeBackgroundCompletionSource);
  const output = validateGeneratedProject("native-background-context", result.artifacts, { run: true });
  assert.match(output.stdout, /native completion/u);
});

test("generated asynchronous Node driver consumes the same component context", { timeout: 300_000 }, () => {
  const result = compile(nativeBackgroundAsyncSource);
  const output = validateGeneratedProject("native-background-async-context", result.artifacts, { run: true });
  assert.match(output.stdout, /native async completion/u);
});

test("generated source compression callback retains its original Error identity", { timeout: 300_000 }, () => {
  const result = compile(nativeBackgroundOriginalErrorSource, "lib");
  const roots = result.artifacts.filter(row => row.path.endsWith(".rs") && /pub static __tsonic_dispatch_/u.test(row.text));
  assert.equal(roots.length, 1, "one component-owned native root module");
  const root = roots[0];
  const name = /pub static (__tsonic_dispatch_\d+)/u.exec(root.text)?.[1];
  assert.equal(typeof name, "string", "exact generated native root declaration");
  const directory = writeGeneratedProject("native-background-original-error", result.artifacts);
  appendFileSync(join(directory, root.path), `
#[cfg(test)]
mod retained_source_failure {
    use tsonic_rust_runtime::ErrorObject;
    #[test]
    fn original_error_survives_native_worker_completion() {
        let expected = crate::schedule().expect("callback registration");
        let result = super::${name}.with(|root| {
            tsonic_rust_node::run_with_contexts(tsonic_rust_node::dispatch::prepend(
                root, tsonic_rust_node::dispatch::DispatchEnd::<tsonic_rust_runtime::TsonicError>::new(),
            ))
        });
        let returned = result.expect_err("source callback must fail");
        assert_eq!(returned.source_error().error_identity_key(), expected.error_identity_key());
        assert_eq!(returned.source_error().error_message(), expected.error_message());
    }
}
`);
  runCargo(directory, ["generate-lockfile", "--offline"]);
  runCargo(directory, ["fmt", "--all"]);
  runCargo(directory, ["fmt", "--all", "--check"]);
  runCargo(directory, ["check", "--all-targets", "--locked", "--offline"]);
  runCargo(directory, ["clippy", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"]);
  runCargo(directory, ["test", "--locked", "--offline"]);
});

test("unused background APIs emit no native context storage", { timeout: 300_000 }, () => {
  const result = compile(nativeBackgroundNoDemandSource);
  assert.equal(result.artifacts.filter(row => row.path.endsWith(".rs"))
    .some(row => /pub static __tsonic_dispatch_/u.test(row.text)), false, "no undemanded physical roots");
  validateGeneratedProject("native-background-no-demand", result.artifacts, { run: true });
});

test("component background roots dispatch through exact two-hop package error domains", { timeout: 300_000 }, () => {
  assert.deepEqual(nativeBackgroundPackageGraph.components.find(row => row.id === "source-package-component:root").dependencies,
    ["source-package-component:@acme/middle"]);
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "bin", crateName: "native_package_background" } },
    sourcePackages: nativeBackgroundPackageGraph, files: nativeBackgroundPackageFiles,
  });
  assert.equal(result.diagnostics.length, 0,
    result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
  assert.equal(result.artifacts.filter(row => row.path.endsWith(".rs") && /pub static __tsonic_dispatch_/u.test(row.text)).length,
    3, "one native background owner per demanding source component");
  const output = validateGeneratedProject("native-background-package-domains", result.artifacts, { run: true });
  for (const name of ["Root", "Middle", "Leaf"]) assert.match(output.stdout, new RegExp(`${name} completion`, "u"));
});
