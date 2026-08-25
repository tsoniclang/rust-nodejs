import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  createRustProviderPackage,
  emptyRustGenerics,
} from "@tsonic/target-rust/provider";
import type {
  RustProviderPackageImplementation,
  RustProviderTypeDefinition,
  RustTargetTypeRef,
} from "@tsonic/target-rust/provider";
import {
  bufferCarrier,
  hashCarrier,
  hmacCarrier,
  httpIncomingMessageCarrier,
  httpServerCarrier,
  httpServerResponseCarrier,
  makeDirectoryOptionsCarrier,
  processEnvCarrier,
  processMemoryUsageCarrier,
  processWriteStreamCarrier,
  rmOptionsCarrier,
  searchParamsCarrier,
  statsCarrier,
  timeoutCarrier,
  textDecoderCarrier,
  urlCarrier,
  urlObjectCarrier,
} from "./model.js";
import { assertModule, assertRows } from "./modules/assert.js";
import { bufferModule, bufferRows } from "./modules/buffer.js";
import { cryptoModule, cryptoRows } from "./modules/crypto.js";
import { childProcessModule, childProcessRows } from "./modules/child-process.js";
import { fsModule, fsRows } from "./modules/filesystem.js";
import {
  fsPromisesModule,
  fsPromisesRows,
} from "./modules/filesystem-promises.js";
import { httpModule, httpRows } from "./modules/http.js";
import { osModule, osRows } from "./modules/os.js";
import { pathModule, pathRows } from "./modules/path.js";
import { processModule, processRows } from "./modules/process.js";
import { timersModule, timersRows } from "./modules/timers.js";
import { urlModule, urlRows } from "./modules/url.js";
import { utilModule, utilRows } from "./modules/util.js";

// Compiled layout is dist/provider/package.js, so the installed package root
// is two directories up from this module.
const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

function rustNodeStruct(
  exportId: string,
  targetCarrier: RustTargetTypeRef,
  objectLiteralConstruction = false,
): RustProviderTypeDefinition {
  return {
    exportId,
    targetDeclarationKind: "struct",
    sourceGenericBindings: [],
    targetGenerics: emptyRustGenerics,
    targetCarrier,
    ...(objectLiteralConstruction
      ? { objectLiteralConstruction: { kind: "struct-default" } as const }
      : {}),
  };
}

export function createRustNodejsProviderPackage(): RustProviderPackageImplementation {
  return createRustProviderPackage({
    id: "@tsonic/rust-nodejs",
    displayName: "Node.js for Rust",
    version: "0.0.1",
    moduleAliases: [
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
      { moduleSpecifier: "path", canonicalModuleSpecifier: "node:path" },
      { moduleSpecifier: "process", canonicalModuleSpecifier: "node:process" },
      { moduleSpecifier: "timers", canonicalModuleSpecifier: "node:timers" },
      { moduleSpecifier: "util", canonicalModuleSpecifier: "node:util" },
      { moduleSpecifier: "url", canonicalModuleSpecifier: "node:url" },
    ],
    modules: [
      assertModule(),
      pathModule(),
      osModule(),
      fsModule(),
      fsPromisesModule(),
      processModule(),
      bufferModule(),
      childProcessModule(),
      urlModule(),
      cryptoModule(),
      utilModule(),
      httpModule(),
      timersModule(),
    ],
    types: [
      rustNodeStruct("node:fs::Stats", statsCarrier),
      rustNodeStruct("node:fs::MakeDirectoryOptions", makeDirectoryOptionsCarrier, true),
      rustNodeStruct("node:fs::RmOptions", rmOptionsCarrier, true),
      rustNodeStruct("node:process::ProcessEnv", processEnvCarrier),
      rustNodeStruct("node:process::MemoryUsage", processMemoryUsageCarrier),
      rustNodeStruct("node:process::ProcessWriteStream", processWriteStreamCarrier),
      rustNodeStruct("node:buffer::Buffer", bufferCarrier),
      rustNodeStruct("node:url::URL", urlCarrier),
      rustNodeStruct("node:url::UrlObject", urlObjectCarrier),
      rustNodeStruct("node:url::Url", urlObjectCarrier),
      rustNodeStruct("node:url::UrlWithStringQuery", urlObjectCarrier),
      rustNodeStruct("node:url::URLSearchParams", searchParamsCarrier),
      rustNodeStruct("node:crypto::Hash", hashCarrier),
      rustNodeStruct("node:crypto::Hmac", hmacCarrier),
      rustNodeStruct("node:http::IncomingMessage", httpIncomingMessageCarrier),
      rustNodeStruct("node:http::ServerResponse", httpServerResponseCarrier),
      rustNodeStruct("node:http::Server", httpServerCarrier),
      rustNodeStruct("node:timers::Timeout", timeoutCarrier),
      rustNodeStruct("node:util::TextDecoder", textDecoderCarrier),
    ],
    operations: [
      ...assertRows(),
      ...pathRows(),
      ...osRows(),
      ...fsRows(),
      ...fsPromisesRows(),
      ...processRows(),
      ...bufferRows(),
      ...childProcessRows(),
      ...urlRows(),
      ...cryptoRows(),
      ...utilRows(),
      ...httpRows(),
      ...timersRows(),
    ],
    aliasImports: [
      { alias: "node_assert", path: "tsonic_rust_node::assert" },
      { alias: "node_path", path: "tsonic_rust_node::path" },
      { alias: "node_os", path: "tsonic_rust_node::os" },
      { alias: "node_fs", path: "tsonic_rust_node::fs" },
      { alias: "node_fs_promises", path: "tsonic_rust_node::fs_promises" },
      { alias: "node_process", path: "tsonic_rust_node::process" },
      { alias: "node_buffer", path: "tsonic_rust_node::buffer" },
      { alias: "node_child_process", path: "tsonic_rust_node::child_process" },
      { alias: "node_url", path: "tsonic_rust_node::url" },
      { alias: "node_crypto", path: "tsonic_rust_node::crypto" },
      { alias: "node_util", path: "tsonic_rust_node::util" },
      { alias: "node_http", path: "tsonic_rust_node::http" },
      { alias: "node_timers", path: "tsonic_rust_node::timers" },
    ],
    binaryEpilogues: [{
      id: "node-event-loop",
      path: "tsonic_rust_node::run_event_loop",
      requiredCrate: "tsonic_rust_node",
      isFallible: true,
      errorBoundary: "source-program",
    }, {
      id: "node-process-exit-code",
      path: "tsonic_rust_node::process::apply_exit_code",
      requiredCrate: "tsonic_rust_node",
    }],
    crates: [{
      crateName: "tsonic_rust_node",
      cargoPath: resolve(packageRoot, "rust/crates/tsonic_rust_node"),
      registryPatch: "crates-io",
    }],
  });
}
