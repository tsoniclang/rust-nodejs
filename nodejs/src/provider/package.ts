import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createRustProviderPackage } from "@tsonic/target-rust/provider";
import type { RustProviderPackageImplementation } from "@tsonic/target-rust/provider";
import {
  bufferCarrier,
  cloneOnlyCarrierTraits,
  hashCarrier,
  hmacCarrier,
  httpIncomingMessageCarrier,
  httpServerCarrier,
  httpServerResponseCarrier,
  processEnvCarrier,
  processMemoryUsageCarrier,
  processWriteStreamCarrier,
  searchParamsCarrier,
  statsCarrier,
  timeoutCarrier,
  urlCarrier,
  urlObjectCarrier,
} from "./model.js";
import { assertModule, assertRows } from "./modules/assert.js";
import { bufferModule, bufferRows } from "./modules/buffer.js";
import { cryptoModule, cryptoRows } from "./modules/crypto.js";
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

export function createRustNodejsProviderPackage(): RustProviderPackageImplementation {
  return createRustProviderPackage({
    id: "@tsonic/rust-nodejs",
    displayName: "Node.js for Rust",
    version: "0.0.1",
    requiredSurfaces: ["js"],
    moduleAliases: [
      { moduleSpecifier: "assert", canonicalModuleSpecifier: "node:assert" },
      { moduleSpecifier: "assert/strict", canonicalModuleSpecifier: "node:assert" },
      { moduleSpecifier: "node:assert/strict", canonicalModuleSpecifier: "node:assert" },
      { moduleSpecifier: "buffer", canonicalModuleSpecifier: "node:buffer" },
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
      urlModule(),
      cryptoModule(),
      utilModule(),
      httpModule(),
      timersModule(),
    ],
    types: [
      { exportId: "node:fs::Stats", targetCarrier: statsCarrier },
      { exportId: "node:process::ProcessEnv", targetCarrier: processEnvCarrier },
      { exportId: "node:process::MemoryUsage", targetCarrier: processMemoryUsageCarrier },
      { exportId: "node:process::ProcessWriteStream", targetCarrier: processWriteStreamCarrier },
      { exportId: "node:buffer::Buffer", targetCarrier: bufferCarrier },
      { exportId: "node:url::URL", targetCarrier: urlCarrier },
      { exportId: "node:url::UrlObject", targetCarrier: urlObjectCarrier },
      { exportId: "node:url::URLSearchParams", targetCarrier: searchParamsCarrier },
      { exportId: "node:crypto::Hash", targetCarrier: hashCarrier },
      { exportId: "node:crypto::Hmac", targetCarrier: hmacCarrier },
      { exportId: "node:http::IncomingMessage", targetCarrier: httpIncomingMessageCarrier },
      { exportId: "node:http::ServerResponse", targetCarrier: httpServerResponseCarrier },
      { exportId: "node:http::Server", targetCarrier: httpServerCarrier },
      { exportId: "node:timers::Timeout", targetCarrier: timeoutCarrier },
    ],
    operations: [
      ...assertRows(),
      ...pathRows(),
      ...osRows(),
      ...fsRows(),
      ...fsPromisesRows(),
      ...processRows(),
      ...bufferRows(),
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
      { alias: "node_url", path: "tsonic_rust_node::url" },
      { alias: "node_crypto", path: "tsonic_rust_node::crypto" },
      { alias: "node_util", path: "tsonic_rust_node::util" },
      { alias: "node_http", path: "tsonic_rust_node::http" },
      { alias: "node_timers", path: "tsonic_rust_node::timers" },
    ],
    carrierPaths: {
      "rust.node.Stats": "tsonic_rust_node::fs::Stats",
      "rust.node.Buffer": "tsonic_rust_node::buffer::Buffer",
      "rust.node.Url": "tsonic_rust_node::url::Url",
      "rust.node.UrlObject": "tsonic_rust_node::url::LegacyUrlObject",
      "rust.node.UrlSearchParams": "tsonic_rust_node::url::UrlSearchParams",
      "rust.node.Hash": "tsonic_rust_node::crypto::Hash",
      "rust.node.Hmac": "tsonic_rust_node::crypto::Hmac",
      "rust.node.ProcessEnv": "tsonic_rust_node::process::ProcessEnv",
      "rust.node.MemoryUsage": "tsonic_rust_node::process::MemoryUsage",
      "rust.node.ProcessWriteStream": "tsonic_rust_node::process::ProcessWriteStream",
      "rust.node.HttpIncomingMessage": "tsonic_rust_node::http::IncomingMessage",
      "rust.node.HttpServerResponse": "tsonic_rust_node::http::ServerResponseHandle",
      "rust.node.HttpServer": "tsonic_rust_node::http::ServerHandle",
      "rust.node.Timeout": "tsonic_rust_node::timers::Timeout",
    },
    carrierTraits: {
      "rust.node.Stats": cloneOnlyCarrierTraits,
      "rust.node.Buffer": cloneOnlyCarrierTraits,
      "rust.node.Url": cloneOnlyCarrierTraits,
      "rust.node.UrlObject": cloneOnlyCarrierTraits,
      "rust.node.UrlSearchParams": cloneOnlyCarrierTraits,
      "rust.node.Hash": cloneOnlyCarrierTraits,
      "rust.node.Hmac": cloneOnlyCarrierTraits,
      "rust.node.MemoryUsage": cloneOnlyCarrierTraits,
      "rust.node.ProcessWriteStream": cloneOnlyCarrierTraits,
      "rust.node.HttpIncomingMessage": cloneOnlyCarrierTraits,
      "rust.node.HttpServerResponse": cloneOnlyCarrierTraits,
      "rust.node.HttpServer": cloneOnlyCarrierTraits,
      "rust.node.Timeout": cloneOnlyCarrierTraits,
    },
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
