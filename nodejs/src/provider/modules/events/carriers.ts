import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const eventEmitterCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.EventEmitter" };

export const mutableEventEmitterCarrier: RustTargetTypeRef = {
  kind: "reference",
  referent: eventEmitterCarrier,
  mutable: true,
};
