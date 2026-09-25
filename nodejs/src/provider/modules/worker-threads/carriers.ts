import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const workerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Worker" };

export const workerOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.WorkerOptions" };

export const messagePortCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.MessagePort" };

export const messageChannelCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.MessageChannel" };
