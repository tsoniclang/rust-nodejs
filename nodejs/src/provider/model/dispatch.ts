import { rustProgramErrorTargetType } from "@tsonic/target-rust/provider";
import type { RustDispatchContextDefinition, RustDispatchContextGroupInput, RustDispatchContextInput } from "@tsonic/target-rust/provider";

const errorArguments = [{ kind: "type" as const, type: rustProgramErrorTargetType() }];

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

export function nodeDispatchGroup(targetArgumentIndex: number): RustDispatchContextGroupInput {
  return {
    contextIds: [nodeBackgroundContext.id, nodeRuntimeTaskContext.id], targetArgumentIndex,
    empty: { form: "associated-call", method: "new", owner: {
      kind: "target-named", id: "rust.node.DispatchEnd", genericArguments: errorArguments,
    } },
    prepend: { form: "call", path: "tsonic_rust_node::dispatch::prepend" },
  };
}
