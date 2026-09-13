import type { RustProviderPackageImplementation } from "@tsonic/target-rust/provider";
import { createRustNodejsProviderPackage } from "./provider/package.js";

export type RustNodejsCapabilityPlugin = RustProviderPackageImplementation;
type CapabilityContext = Parameters<NonNullable<RustNodejsCapabilityPlugin["sourceCompilerContributions"]>>[0];

export function createRustNodejsCapability(): RustNodejsCapabilityPlugin {
  const nativeProfile = createRustNodejsProviderPackage(false);
  const jsProfile = createRustNodejsProviderPackage(true);
  const selectProfile = (context: CapabilityContext) =>
    context.selectedSurfaceIds?.includes("js") === true ? jsProfile : nativeProfile;
  return Object.freeze({
    ...nativeProfile,
    sourceCompilerContributions(context: CapabilityContext) {
      return selectProfile(context).sourceCompilerContributions!(context);
    },
    createTargetContributions(context: CapabilityContext) {
      return selectProfile(context).createTargetContributions!(context);
    },
    sourceProfileContributions(context: CapabilityContext) {
      return {
        declarations: [...(selectProfile(context).sourceProfileContributions?.(context).declarations ?? []), {
          fileName: "node-globals.d.ts",
          text: [
            'declare var process: typeof import("node:process").default;',
            ...(context.selectedSurfaceIds?.includes("js") === true
              ? [
                  'declare var TextEncoder: typeof import("node:util").TextEncoder;',
                ]
              : []),
            'declare var TextDecoder: typeof import("node:util").TextDecoder;',
          ].join("\n"),
        }],
      };
    },
  });
}
