import { rustCallableTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { stringCarrier, unitCarrier } from "../../model/carriers.js";

export const statsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Stats" };

export const makeDirectoryOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.MakeDirectoryOptions" };

export const rmOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.RmOptions" };

export const readStreamCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ReadStream" };

export const writeStreamCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.WriteStream" };

export const readStreamOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.ReadStreamOptions" };

export const writeStreamOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.WriteStreamOptions" };

export const fsWatcherCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.FsWatcher" };

export const fileWatchCallbackCarrier = rustCallableTargetType(
  [stringCarrier, stringCarrier],
  unitCarrier,
);

export const fileStatWatchCallbackCarrier = rustCallableTargetType(
  [statsCarrier, statsCarrier],
  unitCarrier,
);
