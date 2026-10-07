import { emptyCallbackCarrier, int32Carrier, unitCarrier } from "../../model/carriers.js";
import { fnExport, providerRef } from "../../declarations/builders.js";
import { int32Type, voidType } from "../../model/source-types.js";
import { timeoutCarrier } from "./carriers.js";
import { nodeTimerInput } from "../../model/dispatch.js";
import { providerNativeFallibility } from "../../model/operations.js";

import type { RustProviderModuleDefinition, RustProviderOperationDefinition } from "@tsonic/target-rust/provider";
export function timersModule(): RustProviderModuleDefinition {
  const m = "node:timers";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.timers",
    exports: [
      {
        id: `${m}::Timeout`,
        name: "Timeout",
        kind: "class",
        members: [],
      },
      fnExport(m, "setTimeout", [
        {
          name: "callback",
          type: {
            kind: "function",
            id: `${m}.TimeoutCallback`,
            parameters: [],
            returnType: voidType,
          },
        },
        { name: "delay", type: int32Type },
      ], providerRef(m, "Timeout")),
      fnExport(m, "setInterval", [
        {
          name: "callback",
          type: {
            kind: "function",
            id: `${m}.IntervalCallback`,
            parameters: [],
            returnType: voidType,
          },
        },
        { name: "delay", type: int32Type },
      ], providerRef(m, "Timeout")),
      ...["clearTimeout", "clearInterval"].map(name =>
        fnExport(m, name, [{ name: "timeout", type: providerRef(m, "Timeout") }], voidType)),
    ],
  };
}

export function timersRows(): readonly RustProviderOperationDefinition[] {
  return [
    {
      exportId: "node:timers::setTimeout",
      operationKind: "method",
      target: { form: "call", path: "node_timers::set_timeout_callable" },
      dispatchInputs: [nodeTimerInput],
      ...providerNativeFallibility,
      resultCarrier: timeoutCarrier,
      parameterCarriers: [emptyCallbackCarrier, int32Carrier],
    },
    {
      exportId: "node:timers::setInterval",
      operationKind: "method",
      target: { form: "call", path: "node_timers::set_interval_callable" },
      dispatchInputs: [nodeTimerInput],
      ...providerNativeFallibility,
      resultCarrier: timeoutCarrier,
      parameterCarriers: [emptyCallbackCarrier, int32Carrier],
    },
    ...(["clearTimeout", "clearInterval"] as const).map((name): RustProviderOperationDefinition => ({
      exportId: `node:timers::${name}`,
      operationKind: "method",
      target: { form: "call", path: `node_timers::${name === "clearTimeout" ? "clear_timeout" : "clear_interval"}`, argModes: ["mut-ref"] },
      resultCarrier: unitCarrier,
      parameterCarriers: [timeoutCarrier],
    })),
  ];
}
