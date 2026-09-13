import {
  float64Carrier, numberType, propertyMember, providerNativeFallibility,
  providerRef, unitCarrier,
} from "../model.js";
import type { ProviderTypeExpr, RustProviderModuleDefinition, RustProviderOperationDefinition, RustTargetTypeRef } from "../model.js";

const moduleSpecifier = "node:process";
const cpuId = `${moduleSpecifier}::CpuUsage`;
const defaultId = "node:process.default";

export const processCpuCarrier: RustTargetTypeRef = { kind: "target-named", id: "rust.node.CpuUsage" };

export const processPlatformType: ProviderTypeExpr = {
  kind: "union",
  types: ["aix", "android", "darwin", "freebsd", "haiku", "linux", "openbsd", "sunos", "win32", "cygwin", "netbsd"]
    .map(value => ({ kind: "literal", value })),
};
export const processArchitectureType: ProviderTypeExpr = {
  kind: "union",
  types: ["arm", "arm64", "ia32", "loong64", "mips", "mipsel", "ppc64", "riscv64", "s390x", "x64"]
    .map(value => ({ kind: "literal", value })),
};

function cpuSignatures(id: string) {
  return [
    { id: `${id}()`, parameters: [], returnType: providerRef(moduleSpecifier, "CpuUsage") },
    { id: `${id}(previous)`, parameters: [{ name: "previous", type: providerRef(moduleSpecifier, "CpuUsage") }], returnType: providerRef(moduleSpecifier, "CpuUsage") },
  ];
}

export function processMetricExports(): RustProviderModuleDefinition["exports"] {
  return [{
    id: cpuId, name: "CpuUsage", kind: "interface",
    members: ["user", "system"].map(name => propertyMember(cpuId, name, numberType, { readonly: false })),
  }, {
    id: `${moduleSpecifier}::cpuUsage`, name: "cpuUsage", kind: "function", signatures: cpuSignatures(`${moduleSpecifier}::cpuUsage`),
  }];
}

export function processMetricMembers() {
  return [{ id: `${defaultId}.cpuUsage`, name: "cpuUsage", kind: "method" as const, static: true, signatures: cpuSignatures(`${defaultId}.cpuUsage`) }];
}

export function processMetricRows(): readonly RustProviderOperationDefinition[] {
  return [
    ...[`${moduleSpecifier}::cpuUsage`, `${defaultId}.cpuUsage`].flatMap(id => [false, true].map(previous => ({
      exportId: id.startsWith(`${defaultId}.`) ? defaultId : id,
      ...(id.startsWith(`${defaultId}.`) ? { memberId: id } : {}),
      signatureId: `${id}(${previous ? "previous" : ""})`,
      operationKind: "method" as const,
      target: { form: "call" as const, path: `node_process::cpu_usage_${previous ? "since" : "current"}` },
      resultCarrier: processCpuCarrier, parameterCarriers: previous ? [processCpuCarrier] : [],
      ...providerNativeFallibility,
    }))),
    ...["user", "system"].flatMap(name => [{
      exportId: cpuId, memberId: `${cpuId}.${name}`, operationKind: "property" as const,
      target: { form: "field" as const, name }, resultCarrier: float64Carrier, receiverCarrier: processCpuCarrier,
    }, {
      exportId: cpuId, memberId: `${cpuId}.${name}`, operationKind: "property-set" as const,
      target: { form: "field" as const, name }, resultCarrier: unitCarrier, receiverCarrier: processCpuCarrier, parameterCarriers: [float64Carrier],
    }]),
  ];
}
