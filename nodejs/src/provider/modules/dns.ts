import {
  dnsAddressArrayCallbackCarrier,
  dnsLookupAddressCarrier,
  dnsLookupCallbackCarrier,
  float64Carrier,
  fnExport,
  numberType,
  propertyMember,
  providerNativeFallibility,
  providerRef,
  stringArrayCarrier,
  stringArrayType,
  stringCarrier,
  stringType,
  unitCarrier,
  voidType,
} from "../model.js";
import type {
  ProviderTypeExpr,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";

const dnsModuleSpecifier = "node:dns";
const dnsPromisesModuleSpecifier = "node:dns/promises";
const lookupAddressId = `${dnsModuleSpecifier}::LookupAddress`;
const anyType = { kind: "any" } as const;
const lookupCallbackType: ProviderTypeExpr = {
  kind: "function",
  id: `${dnsModuleSpecifier}.LookupCallback`,
  parameters: [
    { name: "error", type: anyType },
    { name: "address", type: stringType },
    { name: "family", type: numberType },
  ],
  returnType: voidType,
};
const addressArrayCallbackType: ProviderTypeExpr = {
  kind: "function",
  id: `${dnsModuleSpecifier}.AddressArrayCallback`,
  parameters: [
    { name: "error", type: anyType },
    { name: "addresses", type: stringArrayType },
  ],
  returnType: voidType,
};

function lookupAddressDeclaration() {
  return {
    id: lookupAddressId,
    name: "LookupAddress",
    kind: "interface" as const,
    members: [
      propertyMember(lookupAddressId, "address", stringType),
      propertyMember(lookupAddressId, "family", numberType),
    ],
  };
}

export function dnsModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier: dnsModuleSpecifier,
    providerModuleId: "tsonic.rust.node.dns",
    exports: [
      lookupAddressDeclaration(),
      fnExport(dnsModuleSpecifier, "lookup", [
        { name: "hostname", type: stringType },
        { name: "callback", type: lookupCallbackType },
      ], voidType),
      ...(["resolve4", "resolve6", "reverse"] as const).map((name) =>
        fnExport(dnsModuleSpecifier, name, [
          { name: name === "reverse" ? "address" : "hostname", type: stringType },
          { name: "callback", type: addressArrayCallbackType },
        ], voidType)),
    ],
  };
}

export function dnsPromisesModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier: dnsPromisesModuleSpecifier,
    providerModuleId: "tsonic.rust.node.dns-promises",
    imports: [{
      moduleSpecifier: dnsModuleSpecifier,
      namedImports: [{ exportedName: "LookupAddress" }],
    }],
    exports: [
      fnExport(dnsPromisesModuleSpecifier, "lookup", [
        { name: "hostname", type: stringType },
      ], providerRef(dnsModuleSpecifier, "LookupAddress")),
      ...(["resolve4", "resolve6", "reverse"] as const).map((name) =>
        fnExport(dnsPromisesModuleSpecifier, name, [
          { name: name === "reverse" ? "address" : "hostname", type: stringType },
        ], stringArrayType)),
    ],
  };
}

export function dnsRows(): readonly RustProviderOperationDefinition[] {
  const callbackRow = (
    name: string,
    callbackCarrier: typeof dnsLookupCallbackCarrier | typeof dnsAddressArrayCallbackCarrier,
  ): RustProviderOperationDefinition => ({
    exportId: `${dnsModuleSpecifier}::${name}`,
    operationKind: "method",
    target: {
      form: "call",
      path: `node_dns::${name === "lookup" ? "lookup" : name}_callable`,
      argModes: ["ref", "value"],
    },
    resultCarrier: unitCarrier,
    parameterCarriers: [stringCarrier, callbackCarrier],
    ...providerNativeFallibility,
  });
  return [
    callbackRow("lookup", dnsLookupCallbackCarrier),
    callbackRow("resolve4", dnsAddressArrayCallbackCarrier),
    callbackRow("resolve6", dnsAddressArrayCallbackCarrier),
    callbackRow("reverse", dnsAddressArrayCallbackCarrier),
    {
      exportId: lookupAddressId,
      memberId: `${lookupAddressId}.address`,
      operationKind: "property",
      target: { form: "receiver-method", name: "address_value" },
      resultCarrier: stringCarrier,
      receiverCarrier: dnsLookupAddressCarrier,
    },
    {
      exportId: lookupAddressId,
      memberId: `${lookupAddressId}.family`,
      operationKind: "property",
      target: { form: "receiver-method", name: "family_number" },
      resultCarrier: float64Carrier,
      receiverCarrier: dnsLookupAddressCarrier,
    },
    {
      exportId: `${dnsPromisesModuleSpecifier}::lookup`,
      operationKind: "method",
      target: { form: "call", path: "node_dns::lookup_async", argModes: ["ref"] },
      resultCarrier: dnsLookupAddressCarrier,
      parameterCarriers: [stringCarrier],
      ...providerNativeFallibility,
      isAsync: true,
    },
    ...(["resolve4", "resolve6", "reverse"] as const).map((name): RustProviderOperationDefinition => ({
      exportId: `${dnsPromisesModuleSpecifier}::${name}`,
      operationKind: "method",
      target: { form: "call", path: `node_dns::${name}_async`, argModes: ["ref"] },
      resultCarrier: stringArrayCarrier,
      parameterCarriers: [stringCarrier],
      ...providerNativeFallibility,
      isAsync: true,
    })),
  ];
}
