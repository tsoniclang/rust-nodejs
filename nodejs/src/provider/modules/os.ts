import { fnExport } from "../declarations/builders.js";
import { providerNativeFallibility } from "../model/operations.js";
import { rustBorrowedStrToStringValueConversion } from "@tsonic/target-rust/provider";
import { stringCarrier } from "../model/carriers.js";
import { stringType } from "../model/source-types.js";

import type { RustProviderModuleDefinition, RustProviderOperationDefinition } from "@tsonic/target-rust/provider";
export function osModule(): RustProviderModuleDefinition {
  const m = "node:os";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.os",
    exports: [
      fnExport(m, "endianness", [], stringType),
      fnExport(m, "platform", [], stringType),
      fnExport(m, "arch", [], stringType),
      fnExport(m, "eol", [], stringType),
      fnExport(m, "hostname", [], stringType),
      fnExport(m, "tmpdir", [], stringType),
      fnExport(m, "homedir", [], stringType),
    ],
  };
}

export function osRows(): readonly RustProviderOperationDefinition[] {
  const call = (name: string, path: string): RustProviderOperationDefinition => ({
    exportId: `node:os::${name}`,
    operationKind: "method",
    target: { form: "call", path },
    resultCarrier: stringCarrier,
  });
  return [
    { exportId: "node:os::endianness", operationKind: "method", target: { form: "call", path: "node_os::endianness" }, resultCarrier: stringCarrier, resultConversion: rustBorrowedStrToStringValueConversion },
    call("platform", "node_os::platform"),
    call("arch", "node_os::arch"),
    { exportId: "node:os::eol", operationKind: "method", target: { form: "call", path: "node_os::eol" }, resultCarrier: stringCarrier, resultConversion: rustBorrowedStrToStringValueConversion },
    call("hostname", "node_os::hostname"),
    {
      exportId: "node:os::tmpdir",
      operationKind: "method",
      target: { form: "call", path: "node_os::tmpdir" },
      resultCarrier: stringCarrier,
      ...providerNativeFallibility,
    },
    { exportId: "node:os::homedir", operationKind: "method", target: { form: "call", path: "node_os::homedir" }, resultCarrier: stringCarrier, ...providerNativeFallibility },
  ];
}

// --- node:fs -----------------------------------------------------------------
