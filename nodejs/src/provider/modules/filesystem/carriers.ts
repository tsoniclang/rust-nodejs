import { rustCallableTargetType } from "@tsonic/target-rust/provider";
import { rustNamedTargetType } from "@tsonic/target-rust/provider";
import type { RustTargetTypeRef } from "@tsonic/target-rust/provider";
import { readableCarrier, streamCarrier, writableCarrier } from "../stream/carriers.js";
import { stringCarrier, unitCarrier } from "../../model/carriers.js";

export const statsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Stats" };

export const makeDirectoryOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.MakeDirectoryOptions" };

export const rmOptionsCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.RmOptions" };

export const readStreamCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.ReadStream",
  "tsonic_rust_node::fs::ReadStream",
  [],
  [],
  undefined,
  [
    { target: streamCarrier, path: "tsonic_rust_node::fs::read_stream_as_stream" },
    { target: readableCarrier, path: "tsonic_rust_node::fs::read_stream_as_readable" },
  ],
);

export const writeStreamCarrier: RustTargetTypeRef = rustNamedTargetType(
  "rust.node.WriteStream",
  "tsonic_rust_node::fs::WriteStream",
  [],
  [],
  undefined,
  [
    { target: streamCarrier, path: "tsonic_rust_node::fs::write_stream_as_stream" },
    { target: writableCarrier, path: "tsonic_rust_node::fs::write_stream_as_writable" },
  ],
);

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
