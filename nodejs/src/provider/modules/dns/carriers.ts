import { rustCallableTargetType, rustOptionTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { stringCarrier, uint8Carrier, stringArrayCarrier, nodeErrorCarrier, unitCarrier } from "../../model/carriers.js";

export const dnsLookupAddressCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.DnsLookupAddress" };

export const dnsLookupCallbackCarrier = rustCallableTargetType(
  [rustOptionTargetType(nodeErrorCarrier), stringCarrier, uint8Carrier],
  unitCarrier,
);

export const dnsAddressArrayCallbackCarrier = rustCallableTargetType(
  [rustOptionTargetType(nodeErrorCarrier), stringArrayCarrier],
  unitCarrier,
);
