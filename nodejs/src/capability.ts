import type { RustProviderPackageImplementation } from "@tsonic/target-rust/provider";
import { createRustNodejsProviderPackage } from "./provider/package.js";

export type RustNodejsCapabilityPlugin = RustProviderPackageImplementation;

export function createRustNodejsCapability(): RustNodejsCapabilityPlugin {
  return createRustNodejsProviderPackage();
}
