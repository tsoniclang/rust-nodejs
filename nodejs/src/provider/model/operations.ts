import type { RustTargetTypeRef, RustTargetTraitRef, RustNamedTypeTraitContract } from "@tsonic/target-rust/provider";
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

export const sourceCallbackFallibility = {
  isFallible: true,
  errorBoundary: "source-program",
  nativeErrorCarriers: [nodeErrorCarrier],
} as const;

export function nativeTrait(
  path: string,
  genericArguments: RustTargetTraitRef["genericArguments"] = [],
): RustTargetTraitRef {
  return { kind: "trait-ref", id: path, path, genericArguments, associatedConstraints: [] };
}

export const cloneOnlyCarrierTraits: RustNamedTypeTraitContract = {
  implementations: [{ trait: nativeTrait("core::clone::Clone"), requirements: [] }],
} as const;

export const closedJsValueCarrierTraits: RustNamedTypeTraitContract = {
  implementations: [
    { trait: nativeTrait("core::clone::Clone"), requirements: [] },
    {
      trait: nativeTrait("tsonic_rust_js::value::JsClosedValueCarrier"),
      requirements: [],
    },
  ],
} as const;

export const copyDefaultCarrierTraits: RustNamedTypeTraitContract = {
  implementations: [
    { trait: nativeTrait("core::clone::Clone"), requirements: [] },
    { trait: nativeTrait("core::default::Default"), requirements: [] },
    { trait: nativeTrait("core::marker::Copy"), requirements: [] },
  ],
} as const;

export const cloneDefaultCarrierTraits: RustNamedTypeTraitContract = {
  implementations: [
    { trait: nativeTrait("core::clone::Clone"), requirements: [] },
    { trait: nativeTrait("core::default::Default"), requirements: [] },
  ],
} as const;
