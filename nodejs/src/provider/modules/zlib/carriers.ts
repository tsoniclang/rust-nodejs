import { rustCallableTargetType, rustOptionTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { bufferCarrier } from "../buffer/carriers.js";
import { nodeErrorCarrier, unitCarrier } from "../../model/carriers.js";

export const zlibOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ZlibOptions" };

export const zlibTransformCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ZlibTransform" };

export const zlibCallbackCarrier = rustCallableTargetType(
  [rustOptionTargetType(nodeErrorCarrier), bufferCarrier],
  unitCarrier,
);
