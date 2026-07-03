export { createRustNodejsCapability } from "./capability.js";
export type { RustNodejsCapabilityPlugin } from "./capability.js";
import { createRustNodejsCapability } from "./capability.js";
import type { RustNodejsCapabilityPlugin } from "./capability.js";

// Standard installed-plugin entrypoint: the host imports the package and
// calls createTsonicPlugin() to register the Node.js Rust target capability.
export function createTsonicPlugin(): RustNodejsCapabilityPlugin {
  return createRustNodejsCapability();
}
