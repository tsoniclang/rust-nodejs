import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { nodeErrorCarrier } from "./carriers.js";

export const trueArgument = { kind: "boolean", value: true } as const;

export const noneArgument = { kind: "none" } as const;

export const zeroIntegerArgument = { kind: "integer", value: 0 } as const;

export const providerNativeFallibility: {
  readonly isFallible: true;
  readonly errorBoundary: "provider-native";
  readonly errorCarrier: RustTargetTypeRef;
} = {
  isFallible: true,
  errorBoundary: "provider-native",
  errorCarrier: nodeErrorCarrier,
};

export const cloneOnlyCarrierTraits = {
  implementations: [{ traitPath: "core::clone::Clone", requirements: [] }],
} as const;

export const closedJsValueCarrierTraits = {
  implementations: [
    { traitPath: "core::clone::Clone", requirements: [] },
    {
      traitPath: "tsonic_rust_js::value::JsClosedValueCarrier",
      requirements: [],
    },
  ],
} as const;

export const copyDefaultCarrierTraits = {
  implementations: [
    { traitPath: "core::clone::Clone", requirements: [] },
    { traitPath: "core::default::Default", requirements: [] },
    { traitPath: "core::marker::Copy", requirements: [] },
  ],
} as const;

export const cloneDefaultCarrierTraits = {
  implementations: [
    { traitPath: "core::clone::Clone", requirements: [] },
    { traitPath: "core::default::Default", requirements: [] },
  ],
} as const;
