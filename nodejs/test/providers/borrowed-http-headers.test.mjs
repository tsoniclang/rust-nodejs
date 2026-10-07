import assert from "node:assert/strict";
import test from "node:test";
import { createRustSession, rustSourceDiagnostics } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

test("HTTP header indexer exposes an exact native readonly slice borrowed from its owner", () => {
  const [contribution] = createTsonicPlugin().createTargetContributions({});
  const { modules, operations, types } = contribution.definition;
  const declaration = modules.find(module => module.moduleSpecifier === "node:http")
    .exports.find(entry => entry.name === "IncomingHttpHeaderValues");
  assert.deepEqual(declaration.members[0].signatures[0].returnType, {
    kind: "union", types: [{ kind: "source-global", name: "ReadonlyArray", typeArguments: [{ kind: "string" }] }, { kind: "undefined" }],
  });
  const row = operations.find(operation => operation.exportId === declaration.id && operation.operationKind === "indexer");
  assert.equal(row.target.name, "get_values");
  assert.deepEqual(row.resultCarrier, {
    kind: "target-named", id: "rust.std.Option", genericArguments: [{ kind: "type", type: {
      kind: "reference", mutable: false, referent: {
        kind: "slice", element: { kind: "target-named", id: "rust.std.String" },
      },
    } }],
  });
  const owners = types.filter(type => type.exportId === declaration.id);
  assert.equal(owners.length, 1);
  assert.deepEqual(owners[0].targetCarrier, row.receiverCarrier);
  const snapshot = operations.find(operation => operation.target.name === "get_all" && operation.exportId === "node:http::IncomingHttpHeaders");
  assert.equal(snapshot.resultCarrier.id, "rust.js.JsArray");
});

test("borrowed distinct headers remain readable without manufacturing a mutable array", () => {
  const diagnostics = rustSourceDiagnostics(createRustSession({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    files: { "index.ts": `import type { IncomingHttpHeaderValues } from "node:http";
      export function first(headers: IncomingHttpHeaderValues): string | undefined {
        const values = headers["accept"];
        return values === undefined ? undefined : values[0];
      }` },
  }));
  assert.equal(diagnostics, "", "exact readonly header values support guarded source reads");
});

test("borrowed distinct headers reject writes, mutable operations and mutable aliases", () => {
  for (const [operation, expected] of [
    ["values[0] = 'changed';", /only permits reading/u],
    ["values.push('changed');", /Property 'push' does not exist/u],
    ["const mutable: string[] = values;", /cannot be assigned to the mutable type/u],
  ]) {
    const diagnostics = rustSourceDiagnostics(createRustSession({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
      files: { "index.ts": `import type { IncomingHttpHeaderValues } from "node:http";
        export function invalid(headers: IncomingHttpHeaderValues): void {
          const values = headers["accept"];
          if (values !== undefined) { ${operation} }
        }` },
    }));
    assert.match(diagnostics, expected, operation);
  }
});
