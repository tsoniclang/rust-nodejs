import {
  boolCarrier, emptyCallbackCarrier, numberType,
  providerNativeFallibility, providerRef, stringCarrier, stringType, voidType,
} from "../model.js";
import type { RustProviderModuleDefinition, RustProviderOperationDefinition, RustTargetTypeRef } from "../model.js";

export const processCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.Process" };
const pidCarrier: RustTargetTypeRef = { kind: "type-parameter", name: "Pid" };
const signalCarrier: RustTargetTypeRef = { kind: "type-parameter", name: "Signal" };
const moduleSpecifier = "node:process";
const processId = `${moduleSpecifier}::Process`;
const callbackType = { kind: "function" as const, id: `${processId}.SignalListener`, parameters: [], returnType: voidType };
const returnType = providerRef(moduleSpecifier, "Process");

export function processSignalMembers(): NonNullable<RustProviderModuleDefinition["exports"][number]["members"]> {
  return [{
    id: `${processId}.kill`, name: "kill", kind: "method",
    signatures: [
      { id: `${processId}.kill(pid)`, parameters: [{ name: "pid", type: numberType }], returnType: { kind: "literal", value: true } },
      ...[stringType, numberType].map(type => ({
        id: `${processId}.kill(pid,${type.kind})`,
        parameters: [{ name: "pid", type: numberType }, { name: "signal", type }],
        returnType: { kind: "literal" as const, value: true },
      })),
    ],
  }, ...["once", "removeListener"].map(name => ({
    id: `${processId}.${name}`, name, kind: "method" as const,
    signatures: [{ id: `${processId}.${name}(signal,listener)`, parameters: [
      { name: "signal", type: providerRef(moduleSpecifier, "Signals") },
      { name: "listener", type: callbackType },
    ], returnType }],
  }))];
}

export function processSignalRows(): readonly RustProviderOperationDefinition[] {
  return [
    { exportId: processId, memberId: `${processId}.kill`, signatureId: `${processId}.kill(pid)`, operationKind: "method",
      target: { form: "call", path: "node_process::kill_default" }, parameterCarriers: [pidCarrier], genericParameters: [{ kind: "type", sourceName: "Pid" }], resultCarrier: boolCarrier, ...providerNativeFallibility },
    ...["string", "number"].map(kind => ({
      exportId: processId, memberId: `${processId}.kill`, signatureId: `${processId}.kill(pid,${kind})`, operationKind: "method" as const,
      target: { form: "call" as const, path: `node_process::kill_${kind === "string" ? "named" : "number"}`, argModes: ["value" as const, kind === "string" ? "ref" as const : "value" as const] },
      parameterCarriers: [pidCarrier, kind === "string" ? stringCarrier : signalCarrier], resultCarrier: boolCarrier,
      genericParameters: [{ kind: "type" as const, sourceName: "Pid" }, ...(kind === "string" ? [] : [{ kind: "type" as const, sourceName: "Signal" }])], ...providerNativeFallibility,
    })),
    ...["once", "removeListener"].map(name => ({
      exportId: processId, memberId: `${processId}.${name}`, signatureId: `${processId}.${name}(signal,listener)`, operationKind: "method" as const,
      target: { form: "call" as const, path: `node_process::${name === "once" ? "once" : "remove_listener"}`, argModes: ["ref" as const, "ref" as const] },
      parameterCarriers: [stringCarrier, emptyCallbackCarrier], resultCarrier: processCarrier, ...providerNativeFallibility,
    })),
  ];
}
