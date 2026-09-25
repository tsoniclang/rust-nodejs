import { rustCallableTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { unitCarrier } from "../../model/carriers.js";

export const netSocketCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.NetSocket" };

export const netServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.NetServer" };

export const netConnectionCallbackCarrier = rustCallableTargetType(
  [netSocketCarrier],
  unitCarrier,
);
