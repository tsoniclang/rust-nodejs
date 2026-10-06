import { providerNativeFallibility } from "../../model/operations.js";
import { rustOptionTargetType } from "@tsonic/target-rust/provider";
import type { RustProviderModuleDefinition, RustProviderOperationDefinition } from "@tsonic/target-rust/provider";
import { providerRef } from "../../declarations/builders.js";
import {
  boolCarrier,
  emptyCallbackCarrier,
  int32Carrier,
  stringCarrier,
} from "../../model/carriers.js";
import { booleanType, int32Type, stringType } from "../../model/source-types.js";
import {
  httpAddressInfoCarrier,
  httpCloseCallbackCarrier,
  httpServerAddressCarrier,
  httpServerCarrier,
} from "./carriers.js";
import {
  moduleSpecifier,
  addressInfoId,
  serverAddressId,
  serverId,
  errorType,
  optionalStringType,
  optionalInt32Type,
  emptyListenerCarrier,
  errorListenerCarrier,
  optionalStringCarrier,
  optionalInt32Carrier,
  optionalAddressCarrier,
  callbackType,
  optional,
} from "./types.js";
import {
  eventRows,
  receiver,
  fallibleReceiver,
  field,
  method,
  property,
  eventMembers,
} from "./members.js";

export function serverDeclaration(): RustProviderModuleDefinition["exports"][number] {
  const classType = providerRef(moduleSpecifier, "Server");
  return {
    id: serverId,
    name: "Server",
    kind: "class",
    members: [
      property(serverId, "listening", booleanType),
      {
        id: `${serverId}.listen`,
        name: "listen",
        kind: "method",
        signatures: [
          {
            id: `${serverId}.listen(port,callback)`,
            parameters: [
              { name: "port", type: int32Type },
              { name: "callback", type: callbackType(`${serverId}.listen(port,callback)`, []), optional: true },
            ],
            returnType: classType,
          },
          {
            id: `${serverId}.listen(port,host,callback)`,
            parameters: [
              { name: "port", type: int32Type },
              { name: "hostname", type: stringType },
              { name: "callback", type: callbackType(`${serverId}.listen(port,host,callback)`, []), optional: true },
            ],
            returnType: classType,
          },
          {
            id: `${serverId}.listen(port,host,backlog,callback)`,
            parameters: [
              { name: "port", type: int32Type },
              { name: "hostname", type: stringType },
              { name: "backlog", type: int32Type },
              { name: "callback", type: callbackType(`${serverId}.listen(port,host,backlog,callback)`, []), optional: true },
            ],
            returnType: classType,
          },
          {
            id: `${serverId}.listen(path,callback)`,
            parameters: [
              { name: "path", type: stringType },
              { name: "callback", type: callbackType(`${serverId}.listen(path,callback)`, []), optional: true },
            ],
            returnType: classType,
          },
        ],
      },
      method(serverId, "address", [], optional(providerRef(moduleSpecifier, "ServerAddress"))),
      method(serverId, "close", [{
        name: "callback",
        type: callbackType(`${serverId}.close.callback`, [{ name: "error", type: errorType, optional: true }]),
        optional: true,
      }], classType),
      method(serverId, "ref", [], classType),
      method(serverId, "unref", [], classType),
      ...eventMembers(serverId, classType, [
        ["error", [{ name: "error", type: errorType }]],
        ["listening", []],
        ["close", []],
      ]),
    ],
  };
}

export function addressInfoDeclaration(): RustProviderModuleDefinition["exports"][number] {
  return {
    id: addressInfoId,
    name: "AddressInfo",
    kind: "class",
    members: [
      property(addressInfoId, "address", stringType),
      property(addressInfoId, "family", stringType),
      property(addressInfoId, "port", int32Type),
    ],
  };
}

export function serverAddressDeclaration(): RustProviderModuleDefinition["exports"][number] {
  return {
    id: serverAddressId,
    name: "ServerAddress",
    kind: "class",
    members: [
      property(serverAddressId, "address", optional(providerRef(moduleSpecifier, "AddressInfo"))),
      property(serverAddressId, "path", optionalStringType),
      property(serverAddressId, "port", optionalInt32Type),
    ],
  };
}

export function addressRows(): readonly RustProviderOperationDefinition[] {
  return [
    field(addressInfoId, "address", httpAddressInfoCarrier, stringCarrier),
    field(addressInfoId, "family", httpAddressInfoCarrier, stringCarrier),
    field(addressInfoId, "port", httpAddressInfoCarrier, int32Carrier),
    receiver(serverAddressId, "address", "address", httpServerAddressCarrier, optionalAddressCarrier, [], "property"),
    receiver(serverAddressId, "path", "path", httpServerAddressCarrier, optionalStringCarrier, [], "property"),
    receiver(serverAddressId, "port", "port", httpServerAddressCarrier, optionalInt32Carrier, [], "property"),
  ];
}

export function serverRows(): readonly RustProviderOperationDefinition[] {
  const optionalCallback = rustOptionTargetType(emptyCallbackCarrier);
  const rows: RustProviderOperationDefinition[] = [
    receiver(serverId, "listening", "listening", httpServerCarrier, boolCarrier, [], "property"),
    fallibleReceiver(serverId, "listen", "listen_default_host_optional", httpServerCarrier, httpServerCarrier, [int32Carrier, optionalCallback], `${serverId}.listen(port,callback)`, ["value", "value"], undefined, providerNativeFallibility),
    fallibleReceiver(serverId, "listen", "listen_optional", httpServerCarrier, httpServerCarrier, [int32Carrier, stringCarrier, optionalCallback], `${serverId}.listen(port,host,callback)`, ["value", "ref", "value"], undefined, providerNativeFallibility),
    fallibleReceiver(serverId, "listen", "listen_with_backlog_optional", httpServerCarrier, httpServerCarrier, [int32Carrier, stringCarrier, int32Carrier, optionalCallback], `${serverId}.listen(port,host,backlog,callback)`, ["value", "ref", "value", "value"], undefined, providerNativeFallibility),
    fallibleReceiver(serverId, "listen", "listen_path_optional", httpServerCarrier, httpServerCarrier, [stringCarrier, optionalCallback], `${serverId}.listen(path,callback)`, ["ref", "value"], undefined, providerNativeFallibility),
    receiver(serverId, "address", "address", httpServerCarrier, rustOptionTargetType(httpServerAddressCarrier), []),
    fallibleReceiver(serverId, "close", "close_optional", httpServerCarrier, httpServerCarrier, [rustOptionTargetType(httpCloseCallbackCarrier)], undefined, undefined, undefined, providerNativeFallibility),
    receiver(serverId, "ref", "ref_chain", httpServerCarrier, httpServerCarrier, []),
    receiver(serverId, "unref", "unref_chain", httpServerCarrier, httpServerCarrier, []),
  ];
  rows.push(...eventRows(serverId, httpServerCarrier, [
    ["error", errorListenerCarrier],
    ["listening", emptyListenerCarrier],
    ["close", emptyListenerCarrier],
  ], providerNativeFallibility));
  return rows;
}
