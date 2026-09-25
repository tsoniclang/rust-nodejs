import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const readableCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Readable" };

export const writableCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Writable" };
