import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { rustProgramErrorTargetType } from "@tsonic/target-rust/provider";

const errorArguments = [{ kind: "type" as const, type: rustProgramErrorTargetType() }];

export const workerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Worker", genericArguments: errorArguments };

export const workerOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.WorkerOptions" };

export const messagePortCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.MessagePort", genericArguments: errorArguments };

export const messageChannelCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.MessageChannel", genericArguments: errorArguments };
