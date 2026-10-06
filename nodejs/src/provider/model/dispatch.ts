import { rustProgramErrorTargetType, rustJsTimerDispatchContextId } from "@tsonic/target-rust/provider";
import type { RustDispatchContextDefinition, RustDispatchContextGroupInput, RustDispatchContextInput } from "@tsonic/target-rust/provider";
import { emptyCallbackCarrier } from "./carriers.js";

const errorArguments = [{ kind: "type" as const, type: rustProgramErrorTargetType() }];
export const nodeTimerCallbackCarrier = emptyCallbackCarrier;

export const nodeBackgroundContext: RustDispatchContextDefinition = {
  id: "tsonic.rust.node.background",
  requiredCrate: "tsonic_rust_node",
  rootCarrier: { kind: "target-named", id: "rust.node.BackgroundTasks", genericArguments: errorArguments },
  construct: { form: "call", path: "tsonic_rust_node::background::BackgroundTasks::new", const: true },
  handleCarrier: { kind: "target-named", id: "rust.node.BackgroundHandle", genericArguments: errorArguments },
  handle: { form: "receiver-method", name: "handle" },
  composedContexts: [],
};

export const nodeBackgroundInput: RustDispatchContextInput = {
  contextId: nodeBackgroundContext.id, view: "root", targetArgumentIndex: 0, mode: "ref",
};

export const nodeRuntimeTaskContext: RustDispatchContextDefinition = {
  id: "tsonic.rust.node.runtime-tasks",
  requiredCrate: "tsonic_rust_node",
  rootCarrier: { kind: "target-named", id: "rust.node.RuntimeTasks", genericArguments: errorArguments },
  construct: { form: "call", path: "tsonic_rust_node::runtime_tasks::RuntimeTasks::new", const: true },
  handleCarrier: { kind: "target-named", id: "rust.node.RuntimeTaskHandle", genericArguments: errorArguments },
  handle: { form: "receiver-method", name: "handle" },
  composedContexts: [],
};

export const nodeRuntimeTaskInput: RustDispatchContextInput = {
  contextId: nodeRuntimeTaskContext.id, view: "root", targetArgumentIndex: 0, mode: "ref",
};

export const nodeWorkerContext: RustDispatchContextDefinition = {
  id: "tsonic.rust.node.workers",
  requiredCrate: "tsonic_rust_node",
  rootCarrier: { kind: "target-named", id: "rust.node.WorkerResources", genericArguments: errorArguments },
  construct: { form: "call", path: "tsonic_rust_node::worker_threads::WorkerResources::new", const: true },
  composedContexts: [],
};

export function nodeWorkerInput(targetArgumentIndex: number): RustDispatchContextInput {
  return { contextId: nodeWorkerContext.id, view: "root", targetArgumentIndex, mode: "ref" };
}

export const nodeSignalContext: RustDispatchContextDefinition = {
  id: "tsonic.rust.node.signals",
  requiredCrate: "tsonic_rust_node",
  rootCarrier: { kind: "target-named", id: "rust.node.SignalTasks", genericArguments: errorArguments },
  construct: { form: "call", path: "tsonic_rust_node::process::SignalTasks::new", const: true },
  composedContexts: [],
};

export const nodeSignalInput: RustDispatchContextInput = {
  contextId: nodeSignalContext.id, view: "root", targetArgumentIndex: 0, mode: "ref",
};

export const nodeNetContext: RustDispatchContextDefinition = {
  id: "tsonic.rust.node.net",
  requiredCrate: "tsonic_rust_node",
  rootCarrier: { kind: "target-named", id: "rust.node.NetServers", genericArguments: errorArguments },
  construct: { form: "call", path: "tsonic_rust_node::net::NetServers::new", const: true },
  composedContexts: [],
};

export const nodeNetInput: RustDispatchContextInput = {
  contextId: nodeNetContext.id, view: "root", targetArgumentIndex: 0, mode: "ref",
};

export const nodeWatcherContext: RustDispatchContextDefinition = {
  id: "tsonic.rust.node.watchers",
  requiredCrate: "tsonic_rust_node",
  rootCarrier: { kind: "target-named", id: "rust.node.Watchers", genericArguments: errorArguments },
  construct: { form: "call", path: "tsonic_rust_node::fs::Watchers::new", const: true },
  composedContexts: [],
};

export const nodeWatcherInput: RustDispatchContextInput = {
  contextId: nodeWatcherContext.id, view: "root", targetArgumentIndex: 0, mode: "ref",
};

export const nodeTlsContext: RustDispatchContextDefinition = {
  id: "tsonic.rust.node.tls",
  requiredCrate: "tsonic_rust_node",
  rootCarrier: { kind: "target-named", id: "rust.node.TlsServers", genericArguments: errorArguments },
  construct: { form: "call", path: "tsonic_rust_node::tls::TlsServers::new", const: true },
  composedContexts: [],
};

export const nodeTlsInput: RustDispatchContextInput = {
  contextId: nodeTlsContext.id, view: "root", targetArgumentIndex: 0, mode: "ref",
};

export const nodeTimerContext: RustDispatchContextDefinition = {
  id: "tsonic.rust.node.timers",
  requiredCrate: "tsonic_rust_node",
  rootCarrier: { kind: "target-named", id: "rust.node.Timers", genericArguments: [{ kind: "type", type: nodeTimerCallbackCarrier }] },
  construct: { form: "call", path: "tsonic_rust_node::timers::new", const: true },
  composedContexts: [],
};

export const nodeTimerInput: RustDispatchContextInput = {
  contextId: nodeTimerContext.id, view: "root", targetArgumentIndex: 0, mode: "ref",
};

export function nodeDispatchGroup(targetArgumentIndex: number, jsEnabled: boolean): RustDispatchContextGroupInput {
  return {
    contextIds: [nodeBackgroundContext.id, nodeRuntimeTaskContext.id, nodeTimerContext.id, nodeWorkerContext.id, nodeSignalContext.id, nodeNetContext.id, nodeWatcherContext.id, nodeTlsContext.id,
      ...(jsEnabled ? [rustJsTimerDispatchContextId] : [])], targetArgumentIndex,
    empty: { form: "associated-call", method: "new", owner: {
      kind: "target-named", id: "rust.node.DispatchEnd", genericArguments: errorArguments,
    } },
    prepend: { form: "call", path: "tsonic_rust_runtime::dispatch::prepend" },
  };
}
