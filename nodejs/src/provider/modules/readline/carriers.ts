import { rustCallableTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { stringCarrier, unitCarrier } from "../../model/carriers.js";

export const readlineOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ReadlineOptions" };

export const readlineInterfaceCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ReadlineInterface" };

export const readlineQuestionCallbackCarrier = rustCallableTargetType(
  [stringCarrier],
  unitCarrier,
);
