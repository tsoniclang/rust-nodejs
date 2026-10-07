import { assertNoTargetDiagnostics } from "../../../../tsonic/test/scripts/diagnostic-assertions.mjs";
import assert from "node:assert/strict";
import test from "node:test";
import { artifactText, compileRust } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { validateGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

test("HTTP address properties retain optional native getters and integer ports", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()], files: { "index.ts": `
    import type { ServerAddress, AddressInfo } from "node:http";
    import type { int32 } from "@tsonic/core/types.js";
    export function port(value: ServerAddress): int32 | undefined { return value.port; }
    export function path(value: ServerAddress): string | undefined { return value.path; }
    export function address(value: ServerAddress): AddressInfo | undefined { return value.address; }
  ` } });
  assertNoTargetDiagnostics(result.diagnostics);
  const output = artifactText(result, "src/index.rs");
  assert.match(output, /Option<i32>/u);
  for (const getter of ["port", "path", "address"]) assert.match(output, new RegExp(`\\.${getter}\\(\\)`));
  assert.doesNotMatch(output, /i32_to_f64|as f64/u);
  validateGeneratedProject("native-server-address", result.artifacts);
});
