import assert from "node:assert/strict";
import test from "node:test";
import { createTsonicPlugin } from "../../../dist/index.js";

test("Node operation selectors preserve each exact source member kind", () => {
  const [{ definition }] = createTsonicPlugin().createTargetContributions({});
  const members = new Map(definition.modules.flatMap(module => module.exports.flatMap(declaration =>
    (declaration.members ?? []).map(member => [member.id, member]))));
  for (const row of definition.operations) {
    if (row.memberId === undefined) continue;
    const member = members.get(row.memberId);
    assert.ok(member, row.memberId);
    const expected = row.operationKind === "property-set" ? "property"
      : row.operationKind === "index-set" ? "indexer" : row.operationKind;
    assert.equal(member.kind, expected, row.memberId);
  }
});
