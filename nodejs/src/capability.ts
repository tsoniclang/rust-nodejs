import type { RustProviderPackageImplementation } from "@tsonic/target-rust/provider";
import { createRustNodejsProviderPackage } from "./provider/package.js";

export type RustNodejsCapabilityPlugin = RustProviderPackageImplementation;

export function createRustNodejsCapability(): RustNodejsCapabilityPlugin {
  return Object.freeze({
    ...createRustNodejsProviderPackage(),
    sourceProfileContributions() {
      return {
        declarations: [{
          fileName: "node-globals.d.ts",
          text: 'declare var process: typeof import("node:process").default;',
        }],
      };
    },
  });
}
