import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const urlCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Url" };

export const urlObjectCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.UrlObject" };

export const searchParamsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.UrlSearchParams" };
