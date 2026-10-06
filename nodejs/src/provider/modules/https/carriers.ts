import { nodeProgramErrorArguments } from "../../model/carriers.js";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";

export const httpsServerCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpsServer", genericArguments: nodeProgramErrorArguments };

export const httpsClientRequestCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.HttpsClientRequest", genericArguments: nodeProgramErrorArguments };
