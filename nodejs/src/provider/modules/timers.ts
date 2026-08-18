import {
  emptyCallbackCarrier,
  fnExport,
  int32Carrier,
  int32Type,
  providerRef,
  timeoutCarrier,
  voidType,
} from "../model.js";

import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";
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
    ],
  };
}

export function timersRows(): readonly RustProviderOperationDefinition[] {
  return [
    {
      exportId: "node:timers::setTimeout",
      operationKind: "method",
      target: { form: "call", path: "node_timers::set_timeout_callable" },
      resultCarrier: timeoutCarrier,
      parameterCarriers: [emptyCallbackCarrier, int32Carrier],
    },
    {
      exportId: "node:timers::setInterval",
      operationKind: "method",
      target: { form: "call", path: "node_timers::set_interval_callable" },
      resultCarrier: timeoutCarrier,
      parameterCarriers: [emptyCallbackCarrier, int32Carrier],
    },
  ];
}
