import {
  boolCarrier,
  booleanType,
  noneArgument,
  providerNativeFallibility,
  stringCarrier,
  stringType,
  unitCarrier,
  voidType,
} from "../model.js";

import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";
export function assertModule(): RustProviderModuleDefinition {
  const moduleSpecifier = "node:assert";
  const exportId = `${moduleSpecifier}::ok`;
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.assert",
    exports: [{
      id: exportId,
      name: "ok",
      kind: "function" as const,
      signatures: [
        {
          id: `${exportId}(value)`,
          name: "ok",
          parameters: [{ name: "value", type: booleanType }],
          returnType: voidType,
        },
        {
          id: `${exportId}(value,message)`,
          name: "ok",
          parameters: [
            { name: "value", type: booleanType },
            { name: "message", type: stringType },
          ],
          returnType: voidType,
        },
      ],
    }],
  };
}

export function assertRows(): readonly RustProviderOperationDefinition[] {
  const exportId = "node:assert::ok";
  return [
    {
      exportId,
      signatureId: `${exportId}(value)`,
      operationKind: "method",
      target: {
        form: "call",
        path: "node_assert::ok",
        trailingArguments: [noneArgument],
      },
      resultCarrier: unitCarrier,
      parameterCarriers: [boolCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId,
      signatureId: `${exportId}(value,message)`,
      operationKind: "method",
      target: {
        form: "call",
        path: "node_assert::ok_with_message",
        argModes: ["value", "ref"],
      },
      resultCarrier: unitCarrier,
      parameterCarriers: [boolCarrier, stringCarrier],
      ...providerNativeFallibility,
    },
  ];
}

// --- node:path ---------------------------------------------------------------
