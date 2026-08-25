import {
  bufferCarrier,
  fnExport,
  hashCarrier,
  hmacCarrier,
  int32Carrier,
  methodMember,
  numberType,
  providerNativeFallibility,
  providerRef,
  rustInt32ToUsizeValueConversion,
  stringCarrier,
  stringType,
  unitCarrier,
  voidType,
} from "../model.js";

import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";
export function cryptoModule(): RustProviderModuleDefinition {
  const m = "node:crypto";
  const hashId = "node:crypto::Hash";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.crypto",
    imports: [{ moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] }],
    exports: [
      fnExport(m, "randomUUID", [], stringType),
      fnExport(m, "createHash", [{ name: "algorithm", type: stringType }], providerRef(m, "Hash")),
      {
        id: hashId,
        name: "Hash",
        kind: "class" as const,
        members: [
          {
            id: `${hashId}.update`,
            name: "update",
            kind: "method" as const,
            signatures: [
              {
                id: `${hashId}.update(string)`,
                parameters: [{ name: "value", type: stringType }],
                returnType: providerRef(m, "Hash"),
              },
              {
                id: `${hashId}.update(buffer)`,
                parameters: [{ name: "value", type: providerRef("node:buffer", "Buffer") }],
                returnType: providerRef(m, "Hash"),
              },
            ],
          },
          methodMember(hashId, "digest", [{ name: "encoding", type: stringType }], stringType),
        ],
      },
      fnExport(m, "createHmac", [{ name: "algorithm", type: stringType }, { name: "key", type: stringType }], providerRef(m, "Hmac")),
      {
        id: "node:crypto::Hmac",
        name: "Hmac",
        kind: "class" as const,
        members: [
          methodMember("node:crypto::Hmac", "update", [{ name: "value", type: stringType }], voidType),
          methodMember("node:crypto::Hmac", "digest", [{ name: "encoding", type: stringType }], stringType),
        ],
      },
      fnExport(m, "randomBytes", [{ name: "size", type: numberType }], providerRef("node:buffer", "Buffer")),
    ],
  };
}

export function cryptoRows(): readonly RustProviderOperationDefinition[] {
  const hashId = "node:crypto::Hash";
  return [
    { exportId: "node:crypto::randomUUID", operationKind: "method", target: { form: "call", path: "node_crypto::random_uuid" }, resultCarrier: stringCarrier, ...providerNativeFallibility },
    { exportId: "node:crypto::createHash", operationKind: "method", target: { form: "call", path: "node_crypto::create_hash", argModes: ["ref"] }, resultCarrier: hashCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    {
      exportId: hashId,
      memberId: `${hashId}.update`,
      signatureId: `${hashId}.update(string)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "update_str_owned", argModes: ["ref"], mutatesReceiver: true },
      resultCarrier: hashCarrier,
      parameterCarriers: [stringCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: hashId,
      memberId: `${hashId}.update`,
      signatureId: `${hashId}.update(buffer)`,
      operationKind: "method",
      target: { form: "receiver-method", name: "update_buffer_owned", argModes: ["ref"], mutatesReceiver: true },
      resultCarrier: hashCarrier,
      parameterCarriers: [bufferCarrier],
      ...providerNativeFallibility,
    },
    { exportId: hashId, memberId: `${hashId}.digest`, operationKind: "method", target: { form: "receiver-method", name: "digest_string", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: "node:crypto::createHmac", operationKind: "method", target: { form: "call", path: "node_crypto::create_hmac_str", argModes: ["ref", "ref"] }, resultCarrier: hmacCarrier, parameterCarriers: [stringCarrier, stringCarrier], ...providerNativeFallibility },
    { exportId: "node:crypto::Hmac", memberId: "node:crypto::Hmac.update", operationKind: "method", target: { form: "receiver-method", name: "update_str", argModes: ["ref"], mutatesReceiver: true }, resultCarrier: unitCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: "node:crypto::Hmac", memberId: "node:crypto::Hmac.digest", operationKind: "method", target: { form: "receiver-method", name: "digest_string", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: "node:crypto::randomBytes", operationKind: "method", target: { form: "call", path: "node_crypto::random_bytes", argConversions: [rustInt32ToUsizeValueConversion] }, resultCarrier: bufferCarrier, parameterCarriers: [int32Carrier], ...providerNativeFallibility },
  ];
}

// --- node:util ---------------------------------------------------------------
