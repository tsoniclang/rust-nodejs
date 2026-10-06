import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { rustProgramErrorTargetType } from "@tsonic/target-rust/provider";

export const eventEmitterCarrier: RustTargetTypeRef = {
  kind: "target-named",
  id: "rust.node.EventEmitter",
  genericArguments: [{ kind: "type", type: rustProgramErrorTargetType() }],
};

export const eventEmitterReferenceCarrier: RustTargetTypeRef = {
  kind: "reference",
  referent: eventEmitterCarrier,
  mutable: false,
};
