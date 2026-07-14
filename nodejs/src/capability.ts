import type { RustProviderOperationsMapper, RustProviderPackageImplementation } from "@tsonic/target-rust";
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
  // Standard target-capability contribution hook: Rust operation rows,
  // alias imports, and carrier paths travel as a
  // rust-provider-operations mapper payload.
  createOperationMappers(context: object): readonly RustProviderOperationsMapper[];
  readonly providerPackage: RustProviderPackageImplementation;
}

export function createRustNodejsCapability(): RustNodejsCapabilityPlugin {
  const providerPackage = createRustNodejsProviderPackage();
  return {
    kind: "target-capability",
    id: providerPackage.id,
    targetId: "rust",
    displayName: providerPackage.displayName,
    requiredSurfaces: providerPackage.requiredSurfaces ?? [],
    moduleOwnership: providerPackage.moduleOwnership,
    createExtensions(context: CapabilityContext): CapabilityExtensions {
      return providerPackage.createExtensions(context);
    },
    createOperationMappers(context: object): readonly RustProviderOperationsMapper[] {
      const createMappers = (providerPackage as {
        createOperationMappers?(context: object): readonly RustProviderOperationsMapper[];
      }).createOperationMappers;
      return createMappers === undefined ? [] : createMappers.call(providerPackage, context);
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
