import type { RustProviderPackageImplementation } from "@tsonic/target-rust";
import { createRustNodejsProviderPackage } from "./provider/nodejs-package.js";

// Context and result types are derived from the underlying provider-package
// implementation so delegation stays 1:1 without a direct @tsonic/target-api
// dependency.
type CapabilityContext = Parameters<RustProviderPackageImplementation["createExtensions"]>[0];
type CapabilityExtensions = ReturnType<RustProviderPackageImplementation["createExtensions"]>;
type RuntimeContributionContext = Parameters<NonNullable<RustProviderPackageImplementation["runtimeContributions"]>>[0];
type RuntimeContributions = ReturnType<NonNullable<RustProviderPackageImplementation["runtimeContributions"]>>;

export interface RustNodejsCapabilityPlugin {
  readonly kind: "target-capability";
  readonly id: string;
  readonly targetId: string;
  readonly displayName: string;
  readonly requiredSurfaces: readonly string[];
  readonly moduleOwnership: readonly { readonly specifierPrefix: string }[];
  createExtensions(context: CapabilityContext): CapabilityExtensions;
  runtimeContributions(context: RuntimeContributionContext): RuntimeContributions;
  readonly providerPackage: RustProviderPackageImplementation;
}

export function createRustNodejsCapability(): RustNodejsCapabilityPlugin {
  const providerPackage = createRustNodejsProviderPackage();
  return {
    kind: "target-capability",
    id: "@tsonic/rust-nodejs",
    targetId: "rust",
    displayName: "Node.js for Rust",
    requiredSurfaces: ["js"],
    moduleOwnership: [{ specifierPrefix: "node:" }],
    createExtensions(context: CapabilityContext): CapabilityExtensions {
      return providerPackage.createExtensions(context);
    },
    runtimeContributions(context: RuntimeContributionContext): RuntimeContributions {
      const contribute = providerPackage.runtimeContributions;
      if (contribute === undefined) {
        return { references: [] };
      }
      return contribute.call(providerPackage, context);
    },
    providerPackage,
  };
}
