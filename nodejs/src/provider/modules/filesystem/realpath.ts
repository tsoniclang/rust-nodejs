import {
  bufferCarrier, propertyMember, providerNativeFallibility, providerRef,
  stringCarrier, stringType, unitCarrier,
} from "../../model.js";
import type { ProviderTypeExpr, RustProviderModuleDefinition, RustProviderOperationDefinition, RustTargetTypeRef } from "../../model.js";

const moduleSpecifier = "node:fs";
const callableId = `${moduleSpecifier}::realpathSync`;
const membersId = `${moduleSpecifier}::RealpathSyncMembers`;
const optionsId = `${moduleSpecifier}::BufferEncodingOptions`;
export const bufferEncodingOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.BufferEncodingOptions" };
export const realpathCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.RealpathSync" };
const paths = [
  { name: "path", type: stringType, carrier: stringCarrier, suffix: "" },
  { name: "bufferPath", type: providerRef("node:buffer", "Buffer"), carrier: bufferCarrier, suffix: "_buffer_path" },
];
function signatures(id: string) {
  return paths.flatMap(path => [false, true].map(buffer => ({
    id: `${id}(${path.name}${buffer ? ",options" : ""})`,
    parameters: [{ name: "path", type: path.type },
      ...(buffer ? [{ name: "options", type: providerRef(moduleSpecifier, "BufferEncodingOptions") }] : [])],
    returnType: buffer ? providerRef("node:buffer", "Buffer") : stringType,
  })));
}

export function realpathExports(): RustProviderModuleDefinition["exports"] {
  return [{
    id: optionsId, name: "BufferEncodingOptions", kind: "interface",
    members: [propertyMember(optionsId, "encoding", { kind: "literal", value: "buffer" }, { readonly: false })],
  }, {
    id: membersId, name: "RealpathSyncMembers", kind: "interface",
    members: [{ id: `${membersId}.native`, name: "native", kind: "method", signatures: signatures(`${membersId}.native`) }],
  }, {
    id: callableId, name: "realpathSync", kind: "value",
    type: { kind: "intersection", types: [
      ...signatures(callableId).map(signature => ({ kind: "function" as const, ...signature })),
      providerRef(moduleSpecifier, "RealpathSyncMembers"),
    ] } satisfies ProviderTypeExpr,
  }];
}

export function realpathRows(): readonly RustProviderOperationDefinition[] {
  return [
    { exportId: callableId, operationKind: "property", evaluation: "pure",
      target: { form: "call", path: "node_fs::realpath_function" }, resultCarrier: realpathCarrier },
    ...[false, true].flatMap(native => paths.flatMap(path => [false, true].map(buffer => ({
      exportId: native ? membersId : callableId,
      ...(native ? { memberId: `${membersId}.native` } : {}),
      signatureId: `${native ? `${membersId}.native` : callableId}(${path.name}${buffer ? ",options" : ""})`,
      operationKind: "method" as const,
      target: { form: "call" as const,
        path: `node_fs::realpath_sync${path.suffix}${buffer ? "_bytes" : ""}`,
        argModes: buffer ? ["ref" as const, "value" as const] : ["ref" as const] },
      parameterCarriers: [path.carrier, ...(buffer ? [bufferEncodingOptionsCarrier] : [])],
      resultCarrier: buffer ? bufferCarrier : stringCarrier, ...providerNativeFallibility,
    })))),
    { exportId: optionsId, memberId: `${optionsId}.encoding`, operationKind: "property",
      target: { form: "field", name: "encoding" }, receiverCarrier: bufferEncodingOptionsCarrier, resultCarrier: stringCarrier },
    { exportId: optionsId, memberId: `${optionsId}.encoding`, operationKind: "property-set",
      target: { form: "field", name: "encoding" }, receiverCarrier: bufferEncodingOptionsCarrier,
      parameterCarriers: [stringCarrier], resultCarrier: unitCarrier },
  ];
}
