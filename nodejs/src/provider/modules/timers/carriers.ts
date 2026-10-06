import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { nodeTimerCallbackCarrier } from "../../model/dispatch.js";

export const timeoutCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Timeout",
  genericArguments: [{ kind: "type", type: nodeTimerCallbackCarrier }] };
