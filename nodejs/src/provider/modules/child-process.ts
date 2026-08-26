import {
  bufferCarrier,
  int32Carrier,
  nullType,
  propertyMember,
  providerNativeFallibility,
  providerRef,
  rustOptionTargetType,
  spawnSyncResultCarrier,
  stringArrayType,
  stringCarrier,
  stringType,
  unitCarrier,
} from "../model.js";
import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";

export function childProcessModule(): RustProviderModuleDefinition {
  const moduleSpecifier = "node:child_process";
  const resultId = `${moduleSpecifier}::SpawnSyncReturns`;
  const resultTypeParameter = { kind: "type-parameter", name: "T" } as const;
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.child-process",
    imports: [{ moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] }],
    exports: [
      {
        id: resultId,
        name: "SpawnSyncReturns",
        kind: "interface" as const,
        typeParameters: [{ name: "T" }],
        members: [
          propertyMember(resultId, "stdout", resultTypeParameter, { readonly: false }),
          propertyMember(resultId, "stderr", resultTypeParameter, { readonly: false }),
          propertyMember(
            resultId,
            "status",
            { kind: "union", types: [{ kind: "number" }, nullType] },
            { readonly: false },
          ),
        ],
      },
      {
        id: `${moduleSpecifier}::spawnSync`,
        name: "spawnSync",
        kind: "function" as const,
        signatures: [{
          id: `${moduleSpecifier}::spawnSync(command,args)`,
          name: "spawnSync",
          parameters: [
            { name: "command", type: stringType },
            { name: "args", type: stringArrayType },
          ],
          returnType: providerRef(
            moduleSpecifier,
            "SpawnSyncReturns",
            [providerRef("node:buffer", "Buffer")],
          ),
        }],
      },
    ],
  };
}

export function childProcessRows(): readonly RustProviderOperationDefinition[] {
  const moduleSpecifier = "node:child_process";
  const resultId = `${moduleSpecifier}::SpawnSyncReturns`;
  const argumentsCarrier = {
    kind: "type-parameter",
    name: "Arguments",
  } as const;
  const resultProperty = (
    name: string,
    carrier: RustProviderOperationDefinition["resultCarrier"],
  ): readonly RustProviderOperationDefinition[] => [
    {
      exportId: resultId,
      memberId: `${resultId}.${name}`,
      operationKind: "property",
      target: { form: "field", name },
      resultCarrier: carrier,
    },
    {
      exportId: resultId,
      memberId: `${resultId}.${name}`,
      operationKind: "property-set",
      target: { form: "field", name },
      resultCarrier: unitCarrier,
      parameterCarriers: [carrier],
    },
  ];
  return [
    {
      exportId: `${moduleSpecifier}::spawnSync`,
      operationKind: "method",
      target: { form: "call", path: "node_child_process::spawn_sync_result", argModes: ["ref", "ref"] },
      resultCarrier: spawnSyncResultCarrier,
      parameterCarriers: [stringCarrier, argumentsCarrier],
      genericParameters: [{ kind: "type", sourceName: argumentsCarrier.name }],
      ...providerNativeFallibility,
    },
    ...resultProperty("stdout", bufferCarrier),
    ...resultProperty("stderr", bufferCarrier),
    ...resultProperty("status", rustOptionTargetType(int32Carrier)),
  ];
}
