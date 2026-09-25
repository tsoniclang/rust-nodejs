import { rustCallableTargetType, rustJsArrayTargetType, rustSourcePrimitiveTargetType, rustStringTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const stringCarrier = rustStringTargetType();

export const boolCarrier = rustSourcePrimitiveTargetType("bool");

export const int32Carrier = rustSourcePrimitiveTargetType("int32");

export const int64Carrier = rustSourcePrimitiveTargetType("int64");

export const uint8Carrier = rustSourcePrimitiveTargetType("uint8");

export const uint32Carrier = rustSourcePrimitiveTargetType("uint32");

export const uint64Carrier = rustSourcePrimitiveTargetType("uint64");

export const nativeUintCarrier = rustSourcePrimitiveTargetType("native-uint");

export const nativeIntCarrier = rustSourcePrimitiveTargetType("native-int");

export const float64Carrier = rustSourcePrimitiveTargetType("float64");

export const jsValueCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.js.JsValue" };

export const stringArrayCarrier = rustJsArrayTargetType(stringCarrier);

export const numberArrayCarrier = rustJsArrayTargetType(float64Carrier);

export const nodeErrorCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.NodeError" };

export const unitCarrier: RustTargetTypeRef = { kind: "tuple", elements: [] };

export const emptyCallbackCarrier = rustCallableTargetType([], unitCarrier);

export const oneValueCallbackCarrier = rustCallableTargetType([jsValueCarrier], unitCarrier);

export const twoValueCallbackCarrier = rustCallableTargetType(
  [jsValueCarrier, jsValueCarrier],
  unitCarrier,
);

export const threeValueCallbackCarrier = rustCallableTargetType(
  [jsValueCarrier, jsValueCarrier, jsValueCarrier],
  unitCarrier,
);
