import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const processEnvCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ProcessEnv" };

export const processMemoryUsageCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.MemoryUsage" };
