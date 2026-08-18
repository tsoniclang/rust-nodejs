import {
  boolCarrier,
  booleanType,
  constructorMember,
  fnExport,
  methodMember,
  noneArgument,
  nullType,
  propertyMember,
  providerNativeFallibility,
  providerRef,
  rustOptionTargetType,
  searchParamsCarrier,
  stringCarrier,
  stringType,
  urlCarrier,
  urlObjectCarrier,
  voidType,
} from "../model.js";

import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";
export function urlModule(): RustProviderModuleDefinition {
  const m = "node:url";
  const urlId = "node:url::URL";
  const paramsId = "node:url::URLSearchParams";
  const urlObjectId = "node:url::UrlObject";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.url",
    exports: [
      {
        id: urlId,
        name: "URL",
        kind: "class" as const,
        members: [
          constructorMember(urlId, [{ name: "input", type: stringType }]),
          propertyMember(urlId, "href", stringType),
          propertyMember(urlId, "protocol", stringType),
          propertyMember(urlId, "host", stringType),
          propertyMember(urlId, "hostname", stringType),
          propertyMember(urlId, "port", stringType),
          propertyMember(urlId, "pathname", stringType),
          propertyMember(urlId, "search", stringType),
          propertyMember(urlId, "hash", stringType),
          propertyMember(urlId, "origin", stringType),
        ],
      },
      {
        id: paramsId,
        name: "URLSearchParams",
        kind: "class" as const,
        members: [
          constructorMember(paramsId, [{ name: "init", type: stringType }]),
          methodMember(paramsId, "get", [{ name: "name", type: stringType }], { kind: "union", types: [stringType, nullType] }),
          methodMember(paramsId, "set", [{ name: "name", type: stringType }, { name: "value", type: stringType }], voidType),
          methodMember(paramsId, "append", [{ name: "name", type: stringType }, { name: "value", type: stringType }], voidType),
          methodMember(paramsId, "has", [{ name: "name", type: stringType }], booleanType),
          methodMember(paramsId, "toString", [], stringType),
        ],
      },
      {
        id: urlObjectId,
        name: "UrlObject",
        kind: "class" as const,
        members: [
          propertyMember(urlObjectId, "href", stringType),
          propertyMember(urlObjectId, "protocol", stringType),
          propertyMember(urlObjectId, "host", stringType),
          propertyMember(urlObjectId, "hostname", stringType),
          propertyMember(urlObjectId, "port", stringType),
          propertyMember(urlObjectId, "pathname", stringType),
          propertyMember(urlObjectId, "search", stringType),
          propertyMember(urlObjectId, "hash", stringType),
        ],
      },
      fnExport(m, "pathToFileURL", [{ name: "path", type: stringType }], providerRef(m, "URL")),
      fnExport(m, "fileURLToPath", [{ name: "url", type: providerRef(m, "URL") }], stringType),
      fnExport(m, "canParse", [{ name: "input", type: stringType }], booleanType),
      fnExport(m, "parse", [{ name: "input", type: stringType }], providerRef(m, "UrlObject")),
      fnExport(m, "format", [{ name: "url", type: providerRef(m, "UrlObject") }], stringType),
    ],
  };
}

export function urlRows(): readonly RustProviderOperationDefinition[] {
  const urlId = "node:url::URL";
  const paramsId = "node:url::URLSearchParams";
  const urlObjectId = "node:url::UrlObject";
  const urlProperty = (name: string): RustProviderOperationDefinition => ({
    exportId: urlId,
    memberId: `${urlId}.${name}`,
    operationKind: "property",
    target: { form: "receiver-method", name },
    resultCarrier: stringCarrier,
  });
  const urlObjectProperty = (name: string): RustProviderOperationDefinition => ({
    exportId: urlObjectId,
    memberId: `${urlObjectId}.${name}`,
    operationKind: "property",
    target: { form: "receiver-method", name },
    resultCarrier: stringCarrier,
  });
  return [
    { exportId: urlId, memberId: `${urlId}.constructor`, operationKind: "constructor", target: { form: "call", path: "node_url::Url::parse", argModes: ["ref"], trailingArguments: [noneArgument] }, resultCarrier: urlCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    ...["href", "protocol", "host", "hostname", "port", "pathname", "search", "hash", "origin"].map(urlProperty),
    { exportId: paramsId, memberId: `${paramsId}.constructor`, operationKind: "constructor", target: { form: "call", path: "node_url::UrlSearchParams::new_from", argModes: ["ref"] }, resultCarrier: searchParamsCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: paramsId, memberId: `${paramsId}.get`, operationKind: "method", target: { form: "receiver-method", name: "get", argModes: ["ref"] }, resultCarrier: rustOptionTargetType(stringCarrier), parameterCarriers: [stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.set`, operationKind: "method", target: { form: "receiver-method", name: "set", argModes: ["ref", "ref"], mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [stringCarrier, stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.append`, operationKind: "method", target: { form: "receiver-method", name: "append", argModes: ["ref", "ref"], mutatesReceiver: true }, resultCarrier: { kind: "tuple", elements: [] }, parameterCarriers: [stringCarrier, stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.has`, operationKind: "method", target: { form: "receiver-method", name: "has", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [stringCarrier] },
    { exportId: paramsId, memberId: `${paramsId}.toString`, operationKind: "method", target: { form: "receiver-method", name: "to_string" }, resultCarrier: stringCarrier },
    { exportId: "node:url::pathToFileURL", operationKind: "method", target: { form: "call", path: "node_url::path_to_file_url", argModes: ["ref"] }, resultCarrier: urlCarrier, parameterCarriers: [stringCarrier] },
    { exportId: "node:url::canParse", operationKind: "method", target: { form: "call", path: "node_url::can_parse", argModes: ["ref"], trailingArguments: [noneArgument] }, resultCarrier: boolCarrier, parameterCarriers: [stringCarrier] },
    { exportId: "node:url::fileURLToPath", operationKind: "method", target: { form: "call", path: "node_url::file_url_to_path", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [urlCarrier], ...providerNativeFallibility },
    { exportId: "node:url::parse", operationKind: "method", target: { form: "call", path: "node_url::parse_legacy", argModes: ["ref"] }, resultCarrier: urlObjectCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    ...["href", "protocol", "host", "hostname", "port", "pathname", "search", "hash"].map(urlObjectProperty),
    { exportId: "node:url::format", operationKind: "method", target: { form: "call", path: "node_url::format_legacy", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [urlObjectCarrier] },
  ];
}

// --- node:crypto -------------------------------------------------------------
