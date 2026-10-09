import assert from "node:assert/strict";
import test from "node:test";
import { appendFileSync } from "node:fs";
import { join } from "node:path";
import { compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeOwnershipCostSupport } from "../../../../tsonic-rust/test/helpers/native-ownership-cost.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

test("opaque response completion releases Error guards with handwritten snapshot allocation cost", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "lib", crateName: "native_error_borrow_effects" } },
    files: { "index.ts": `
import type { ServerResponse } from "node:http";
export function send(response: ServerResponse): string {
  const failure = new Error("before");
  response.on("finish", (): void => { failure.message = "after"; });
  response.end(failure.message);
  return failure.message;
}
` } });
  assert.equal(result.diagnostics.length, 0,
    result.diagnostics.slice(0, 6).map(row => row.message.slice(0, 256)).join("\n"));
  const directory = writeGeneratedProject("native-error-borrow-effects", result.artifacts);
  appendFileSync(join(directory, "src/lib.rs"), `
#[cfg(test)]
mod native_borrow_effects {
    use super::program::TsonicError;
    use tsonic_rust_node::http::ServerResponse;
    use tsonic_rust_runtime::{Callable, ErrorObject, MutableJsError, WritableErrorObject};
    ${nativeOwnershipCostSupport}

    fn handwritten(response: ServerResponse<TsonicError>) -> Result<String, TsonicError> {
        let failure = MutableJsError::error("before");
        let captured = failure.clone();
        response.on_finish("finish", &Callable::new(move |()| {
            captured.set_error_message(String::from("after"));
            Ok(())
        }))?;
        let message = String::from(failure.error_message());
        response.end_string(&message)?;
        Ok(String::from(failure.error_message()))
    }

    #[test]
    fn snapshot_precedes_reentrant_mutation_at_native_cost() {
        for _ in 0..100 {
            let generated_response = ServerResponse::<TsonicError>::new();
            let generated_input = generated_response.clone();
            let (actual, generated_cost) = measure(|| crate::send(generated_input).unwrap());
            let native_response = ServerResponse::<TsonicError>::new();
            let native_input = native_response.clone();
            let (expected, native_cost) = measure(|| handwritten(native_input).unwrap());
            assert_eq!(generated_cost, native_cost);
            assert_eq!(actual, "after");
            assert_eq!(actual, expected);
            assert_eq!(generated_response.to_response().body, b"before");
            assert_eq!(native_response.to_response().body, b"before");
        }
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
