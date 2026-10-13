import type { RustProviderModuleAliasDefinition } from "@tsonic/target-rust/provider";

export function nodejsProviderModuleSpecifiers(): readonly RustProviderModuleAliasDefinition[] {
  return [
    { moduleSpecifier: "assert", canonicalModuleSpecifier: "node:assert" },
    { moduleSpecifier: "assert/strict", canonicalModuleSpecifier: "node:assert" },
    { moduleSpecifier: "node:assert/strict", canonicalModuleSpecifier: "node:assert" },
    { moduleSpecifier: "buffer", canonicalModuleSpecifier: "node:buffer" },
    { moduleSpecifier: "child_process", canonicalModuleSpecifier: "node:child_process" },
    { moduleSpecifier: "crypto", canonicalModuleSpecifier: "node:crypto" },
    { moduleSpecifier: "fs", canonicalModuleSpecifier: "node:fs" },
    { moduleSpecifier: "fs/promises", canonicalModuleSpecifier: "node:fs/promises" },
    { moduleSpecifier: "http", canonicalModuleSpecifier: "node:http" },
    { moduleSpecifier: "os", canonicalModuleSpecifier: "node:os" },
    { moduleSpecifier: "v8", canonicalModuleSpecifier: "node:v8" },
    { moduleSpecifier: "path", canonicalModuleSpecifier: "node:path" },
    { moduleSpecifier: "process", canonicalModuleSpecifier: "node:process" },
    { moduleSpecifier: "perf_hooks", canonicalModuleSpecifier: "node:perf_hooks" },
    { moduleSpecifier: "timers", canonicalModuleSpecifier: "node:timers" },
    { moduleSpecifier: "events", canonicalModuleSpecifier: "node:events" },
    { moduleSpecifier: "stream", canonicalModuleSpecifier: "node:stream" },
    { moduleSpecifier: "dns", canonicalModuleSpecifier: "node:dns" },
    { moduleSpecifier: "dns/promises", canonicalModuleSpecifier: "node:dns/promises" },
    { moduleSpecifier: "zlib", canonicalModuleSpecifier: "node:zlib" },
    { moduleSpecifier: "net", canonicalModuleSpecifier: "node:net" },
    { moduleSpecifier: "tls", canonicalModuleSpecifier: "node:tls" },
    { moduleSpecifier: "https", canonicalModuleSpecifier: "node:https" },
    { moduleSpecifier: "readline", canonicalModuleSpecifier: "node:readline" },
    { moduleSpecifier: "worker_threads", canonicalModuleSpecifier: "node:worker_threads" },
    { moduleSpecifier: "util", canonicalModuleSpecifier: "node:util" },
    { moduleSpecifier: "url", canonicalModuleSpecifier: "node:url" },
  ];
}
