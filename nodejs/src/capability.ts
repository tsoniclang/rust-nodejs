import type { RustProviderPackageImplementation } from "@tsonic/target-rust";
import { createRustNodejsProviderPackage } from "./provider/nodejs-package.js";

export type RustNodejsCapabilityPlugin = RustProviderPackageImplementation;

export function createRustNodejsCapability(): RustNodejsCapabilityPlugin {
  return createRustNodejsProviderPackage();
}
