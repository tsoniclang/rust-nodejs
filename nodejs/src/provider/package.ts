import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createRustProviderPackage } from "@tsonic/target-rust/provider";
import type { RustProviderPackageImplementation } from "@tsonic/target-rust/provider";
import { bufferCarrier } from "./modules/buffer/carriers.js";
import { cloneOnlyCarrierTraits, closedJsValueCarrierTraits, cloneDefaultCarrierTraits, copyDefaultCarrierTraits } from "./model/operations.js";
import { hashCarrier, hmacCarrier, cryptoCarrier } from "./modules/crypto/carriers.js";
import {
  httpAddressInfoCarrier,
  httpIncomingMessageCarrier,
  httpServerAddressCarrier,
  httpServerCarrier,
  httpServerResponseCarrier,
  incomingHttpHeadersCarrier,
  outgoingHttpHeadersCarrier,
} from "./modules/http/carriers.js";
import { makeDirectoryOptionsCarrier, rmOptionsCarrier, statsCarrier, readStreamCarrier, readStreamOptionsCarrier, writeStreamCarrier, writeStreamOptionsCarrier, fsWatcherCarrier } from "./modules/filesystem/carriers.js";
import { nodeErrorCarrier } from "./model/carriers.js";
import { processEnvCarrier, processMemoryUsageCarrier } from "./modules/process/carriers.js";
import { searchParamsCarrier, urlCarrier, urlObjectCarrier } from "./modules/url/carriers.js";
import { timeoutCarrier } from "./modules/timers/carriers.js";
import { textDecoderCarrier, textEncoderCarrier } from "./modules/util/carriers.js";
import { eventEmitterCarrier } from "./modules/events/carriers.js";
import {
  duplexCarrier,
  readableCarrier,
  streamCarrier,
  transformCarrier,
  writableCarrier,
} from "./modules/stream/carriers.js";
import { dnsLookupAddressCarrier } from "./modules/dns/carriers.js";
import { brotliOptionsCarrier, zlibOptionsCarrier, zlibTransformCarrier } from "./modules/zlib/carriers.js";
import { netSocketCarrier, netServerCarrier } from "./modules/net/carriers.js";
import { tlsConnectOptionsCarrier, tlsServerOptionsCarrier, tlsSocketCarrier, tlsServerCarrier } from "./modules/tls/carriers.js";
import { httpsServerCarrier, httpsClientRequestCarrier } from "./modules/https/carriers.js";
import { readlineOptionsCarrier, readlineInterfaceCarrier } from "./modules/readline/carriers.js";
import { messageChannelCarrier, messagePortCarrier, workerCarrier, workerOptionsCarrier } from "./modules/worker-threads/carriers.js";
import { assertModule, assertRows } from "./modules/assert.js";
import { bufferModule, bufferRows } from "./modules/buffer/declarations.js";
import { cryptoModule, cryptoRows } from "./modules/crypto/declarations.js";
import { childProcessModule, childProcessRows, spawnOptionsCarrier } from "./modules/child-process/declarations.js";
import { fsModule, fsRows } from "./modules/filesystem/calls.js";
import { statOptionsCarrier, directoryOptionsCarrier, fsConstantsCarrier, direntCarrier, direntGenerics } from "./modules/filesystem/paths.js";
import { bufferEncodingOptionsCarrier } from "./modules/filesystem/realpath.js";
import {
  fsPromisesModule,
  fsPromisesRows,
} from "./modules/filesystem/promises.js";
import { httpModule, httpRows } from "./modules/http/declarations.js";
import { osModule, osRows } from "./modules/os.js";
import { v8HeapInfoCarrier, v8Module, v8Rows } from "./modules/v8.js";
import { pathModule, pathRows } from "./modules/path.js";
import { processModule, processRows } from "./modules/process/declarations.js";
import { processCarrier } from "./modules/process/signals.js";
import { processCpuCarrier } from "./modules/process/metrics.js";
import { performanceCarrier, performanceModule, performanceRows } from "./modules/performance.js";
import { timersModule, timersRows } from "./modules/timers/declarations.js";
import { eventsModule, eventsRows } from "./modules/events/declarations.js";
import { streamModule, streamRows } from "./modules/stream/declarations.js";
import { urlModule, urlRows } from "./modules/url/declarations.js";
import { utilModule, utilRows } from "./modules/util/declarations.js";
import {
  dnsModule,
  dnsPromisesModule,
  dnsRows,
} from "./modules/dns/declarations.js";
import { zlibModule, zlibRows } from "./modules/zlib/declarations.js";
import { netModule, netRows } from "./modules/net/declarations.js";
import { tlsModule, tlsRows } from "./modules/tls/declarations.js";
import { httpsModule, httpsRows } from "./modules/https/declarations.js";
import { readlineModule, readlineRows } from "./modules/readline/declarations.js";
import {
  workerThreadCarrierTraits,
  workerThreadsModule,
  workerThreadsRows,
} from "./modules/worker-threads/declarations.js";

// Compiled layout is dist/provider/package.js, so the installed package root
// is two directories up from this module.
const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const require = createRequire(import.meta.url);
const rustJsPackageRoot = dirname(require.resolve("@tsonic/rust-js/package.json"));

export function createRustNodejsProviderPackage(typedArrays: boolean): RustProviderPackageImplementation {
  const providerPackage = createRustProviderPackage({
    id: "@tsonic/rust-nodejs",
    sourceGlobals: {
      performance: "node:perf_hooks::performance",
      ...(typedArrays ? { crypto: "node:crypto::webcrypto" } : {}),
    },
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
    ],
    modules: [
      assertModule(),
      pathModule(),
      osModule(),
      v8Module(),
      fsModule(typedArrays),
      fsPromisesModule(),
      processModule(),
      performanceModule(),
      bufferModule(typedArrays),
      childProcessModule(typedArrays),
      urlModule(),
      cryptoModule(typedArrays),
      utilModule(typedArrays),
      httpModule(),
      timersModule(),
      eventsModule(),
      streamModule(),
      dnsModule(),
      dnsPromisesModule(),
      zlibModule(typedArrays),
      netModule(),
      tlsModule(),
      httpsModule(),
      readlineModule(),
      workerThreadsModule(),
    ],
    types: [
      { exportId: "node:process::Process", targetCarrier: processCarrier },
      { exportId: "node:child_process::SpawnSyncError", targetCarrier: nodeErrorCarrier },
      { exportId: "node:child_process::SpawnSyncOptionsWithBufferEncoding", targetCarrier: spawnOptionsCarrier, objectLiteralConstruction: { kind: "struct-default" } },
      { exportId: "node:perf_hooks::Performance", targetCarrier: performanceCarrier },
      { exportId: "node:process::CpuUsage", targetCarrier: processCpuCarrier, objectLiteralConstruction: { kind: "struct-default" } },
      { exportId: "node:fs::Stats", targetCarrier: statsCarrier },
      { exportId: "node:fs::StatOptions", targetCarrier: statOptionsCarrier, objectLiteralConstruction: { kind: "struct-default" } },
      { exportId: "node:fs::BufferDirectoryOptions", targetCarrier: directoryOptionsCarrier, objectLiteralConstruction: { kind: "struct-default" } },
      { exportId: "node:fs::BufferEncodingOptions", targetCarrier: bufferEncodingOptionsCarrier, objectLiteralConstruction: { kind: "struct-default" } },
      { exportId: "node:fs::FsConstants", targetCarrier: fsConstantsCarrier },
      { exportId: "node:fs::Dirent", targetCarrier: direntCarrier({ kind: "type-parameter", name: "Name" }), genericParameters: direntGenerics },
      {
        exportId: "node:fs::MakeDirectoryOptions",
        targetCarrier: makeDirectoryOptionsCarrier,
        objectLiteralConstruction: { kind: "struct-default" },
      },
      {
        exportId: "node:fs::RmOptions",
        targetCarrier: rmOptionsCarrier,
        objectLiteralConstruction: { kind: "struct-default" },
      },
      { exportId: "node:process::ProcessEnv", targetCarrier: processEnvCarrier, objectLiteralConstruction: { kind: "default" } },
      { exportId: "node:process::MemoryUsage", targetCarrier: processMemoryUsageCarrier },
      { exportId: "node:v8::HeapInfo", targetCarrier: v8HeapInfoCarrier },
      { exportId: "node:process::ProcessWriteStream", targetCarrier: writableCarrier },
      { exportId: "node:buffer::Buffer", targetCarrier: bufferCarrier },
      { exportId: "node:url::URL", targetCarrier: urlCarrier },
      { exportId: "node:url::UrlObject", targetCarrier: urlObjectCarrier },
      { exportId: "node:url::Url", targetCarrier: urlObjectCarrier },
      { exportId: "node:url::UrlWithStringQuery", targetCarrier: urlObjectCarrier },
      { exportId: "node:url::URLSearchParams", targetCarrier: searchParamsCarrier },
      { exportId: "node:crypto::Hash", targetCarrier: hashCarrier },
      { exportId: "node:crypto::Hmac", targetCarrier: hmacCarrier },
      { exportId: "node:http::IncomingMessage", targetCarrier: httpIncomingMessageCarrier },
      { exportId: "node:http::ServerResponse", targetCarrier: httpServerResponseCarrier },
      { exportId: "node:http::Server", targetCarrier: httpServerCarrier },
      { exportId: "node:http::IncomingHttpHeaders", targetCarrier: incomingHttpHeadersCarrier },
      { exportId: "node:http::OutgoingHttpHeaders", targetCarrier: outgoingHttpHeadersCarrier },
      { exportId: "node:http::AddressInfo", targetCarrier: httpAddressInfoCarrier },
      { exportId: "node:http::ServerAddress", targetCarrier: httpServerAddressCarrier },
      { exportId: "node:timers::Timeout", targetCarrier: timeoutCarrier },
      { exportId: "node:util::TextDecoder", targetCarrier: textDecoderCarrier },
      ...(typedArrays ? [
        { exportId: "node:util::TextEncoder", targetCarrier: textEncoderCarrier },
        { exportId: "node:crypto::Crypto", targetCarrier: cryptoCarrier },
      ] : []),
      { exportId: "node:events::EventEmitter", targetCarrier: eventEmitterCarrier },
      { exportId: "node:stream::Stream", targetCarrier: streamCarrier },
      { exportId: "node:stream::Readable", targetCarrier: readableCarrier },
      { exportId: "node:stream::Writable", targetCarrier: writableCarrier },
      { exportId: "node:stream::Duplex", targetCarrier: duplexCarrier },
      { exportId: "node:stream::Transform", targetCarrier: transformCarrier },
      { exportId: "node:fs::ReadStream", targetCarrier: readStreamCarrier },
      { exportId: "node:fs::WriteStream", targetCarrier: writeStreamCarrier },
      {
        exportId: "node:fs::ReadStreamOptions",
        targetCarrier: readStreamOptionsCarrier,
        objectLiteralConstruction: { kind: "struct-default" },
      },
      {
        exportId: "node:fs::WriteStreamOptions",
        targetCarrier: writeStreamOptionsCarrier,
        objectLiteralConstruction: { kind: "struct-default" },
      },
      { exportId: "node:fs::FSWatcher", targetCarrier: fsWatcherCarrier },
      { exportId: "node:dns::LookupAddress", targetCarrier: dnsLookupAddressCarrier },
      {
        exportId: "node:zlib::ZlibOptions",
        targetCarrier: zlibOptionsCarrier,
        objectLiteralConstruction: { kind: "struct-default" },
      },
      {
        exportId: "node:zlib::BrotliOptions",
        targetCarrier: brotliOptionsCarrier,
        objectLiteralConstruction: { kind: "struct-default" },
      },
      { exportId: "node:zlib::ZlibTransform", targetCarrier: zlibTransformCarrier },
      { exportId: "node:net::Socket", targetCarrier: netSocketCarrier },
      { exportId: "node:net::Server", targetCarrier: netServerCarrier },
      {
        exportId: "node:tls::ConnectionOptions",
        targetCarrier: tlsConnectOptionsCarrier,
        objectLiteralConstruction: { kind: "struct-default" },
      },
      {
        exportId: "node:tls::TlsOptions",
        targetCarrier: tlsServerOptionsCarrier,
        objectLiteralConstruction: { kind: "struct-default" },
      },
      { exportId: "node:tls::TLSSocket", targetCarrier: tlsSocketCarrier },
      { exportId: "node:tls::Server", targetCarrier: tlsServerCarrier },
      {
        exportId: "node:https::ServerOptions",
        targetCarrier: tlsServerOptionsCarrier,
        objectLiteralConstruction: { kind: "struct-default" },
      },
      { exportId: "node:https::Server", targetCarrier: httpsServerCarrier },
      { exportId: "node:https::ClientRequest", targetCarrier: httpsClientRequestCarrier },
      {
        exportId: "node:readline::ReadLineOptions",
        targetCarrier: readlineOptionsCarrier,
        objectLiteralConstruction: { kind: "struct-default" },
      },
      { exportId: "node:readline::Interface", targetCarrier: readlineInterfaceCarrier },
      { exportId: "node:worker_threads::Worker", targetCarrier: workerCarrier },
      {
        exportId: "node:worker_threads::WorkerOptions",
        targetCarrier: workerOptionsCarrier,
        objectLiteralConstruction: { kind: "struct-default" },
      },
      { exportId: "node:worker_threads::MessagePort", targetCarrier: messagePortCarrier },
      { exportId: "node:worker_threads::MessageChannel", targetCarrier: messageChannelCarrier },
    ],
    operations: [
      ...assertRows(),
      ...pathRows(),
      ...osRows(),
      ...v8Rows(),
      ...fsRows(typedArrays),
      ...fsPromisesRows(),
      ...processRows(),
      ...performanceRows(),
      ...bufferRows(typedArrays),
      ...childProcessRows(typedArrays),
      ...urlRows(),
      ...cryptoRows(typedArrays),
      ...utilRows(typedArrays),
      ...httpRows(),
      ...timersRows(),
      ...eventsRows(),
      ...streamRows(),
      ...dnsRows(),
      ...zlibRows(typedArrays),
      ...netRows(),
      ...tlsRows(),
      ...httpsRows(),
      ...readlineRows(),
      ...workerThreadsRows(),
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
      { alias: "node_events", path: "tsonic_rust_node::events" },
      { alias: "node_stream", path: "tsonic_rust_node::stream" },
      { alias: "node_dns", path: "tsonic_rust_node::dns" },
      { alias: "node_zlib", path: "tsonic_rust_node::zlib" },
      { alias: "node_net", path: "tsonic_rust_node::net" },
      { alias: "node_tls", path: "tsonic_rust_node::tls" },
      { alias: "node_https", path: "tsonic_rust_node::https" },
      { alias: "node_readline", path: "tsonic_rust_node::readline" },
      { alias: "node_worker_threads", path: "tsonic_rust_node::worker_threads" },
    ],
    carrierPaths: {
      "rust.node.Stats": "tsonic_rust_node::fs::Stats",
      "rust.node.StatOptions": "tsonic_rust_node::fs::StatOptions",
      "rust.node.BufferDirectoryOptions": "tsonic_rust_node::fs::BufferDirectoryOptions",
      "rust.node.FsConstants": "tsonic_rust_node::fs::FsConstants",
      "rust.node.Dirent": "tsonic_rust_node::fs::Dirent",
      "rust.node.MakeDirectoryOptions": "tsonic_rust_node::fs::MakeDirectoryOptions",
      "rust.node.RmOptions": "tsonic_rust_node::fs::RmOptions",
      "rust.node.BufferEncodingOptions": "tsonic_rust_node::fs::BufferEncodingOptions",
      "rust.node.RealpathSync": "tsonic_rust_node::fs::RealpathSync",
      "rust.node.Buffer": "tsonic_rust_node::buffer::Buffer",
      "rust.node.SpawnSyncResult": "tsonic_rust_node::child_process::SpawnSyncResult",
      "rust.node.SpawnSyncOptions": "tsonic_rust_node::child_process::SpawnSyncOptions",
      "rust.node.Url": "tsonic_rust_node::url::Url",
      "rust.node.UrlObject": "tsonic_rust_node::url::LegacyUrlObject",
      "rust.node.UrlSearchParams": "tsonic_rust_node::url::UrlSearchParams",
      "rust.node.Hash": "tsonic_rust_node::crypto::Hash",
      "rust.node.Hmac": "tsonic_rust_node::crypto::Hmac",
      "rust.node.ProcessEnv": "tsonic_rust_node::process::ProcessEnv",
      "rust.node.Process": "tsonic_rust_node::process::Process",
      "rust.node.MemoryUsage": "tsonic_rust_node::process::MemoryUsage",
      "rust.node.HeapInfo": "tsonic_rust_node::v8::HeapInfo",
      "rust.node.CpuUsage": "tsonic_rust_node::process::CpuUsage",
      "rust.node.Performance": "tsonic_rust_node::perf_hooks::Performance",
      "rust.node.HttpIncomingMessage": "tsonic_rust_node::http::IncomingMessage",
      "rust.node.HttpServerResponse": "tsonic_rust_node::http::ServerResponse",
      "rust.node.HttpServer": "tsonic_rust_node::http::ServerHandle",
      "rust.node.IncomingHttpHeaders": "tsonic_rust_node::http::IncomingHttpHeaders",
      "rust.node.OutgoingHttpHeaders": "tsonic_rust_node::http::OutgoingHttpHeaders",
      "rust.node.HttpAddressInfo": "tsonic_rust_node::http::AddressInfo",
      "rust.node.HttpServerAddress": "tsonic_rust_node::http::ServerAddress",
      "rust.node.Timeout": "tsonic_rust_node::timers::Timeout",
      "rust.node.TextDecoder": "tsonic_rust_node::util::TextDecoder",
      "rust.node.TextEncoder": "tsonic_rust_node::util::TextEncoder",
      "rust.node.Crypto": "tsonic_rust_node::crypto::webcrypto::Crypto",
      "rust.node.EventEmitter": "tsonic_rust_node::events::EventEmitter",
      "rust.node.Stream": "tsonic_rust_node::stream::Stream",
      "rust.node.Readable": "tsonic_rust_node::stream::Readable",
      "rust.node.Writable": "tsonic_rust_node::stream::Writable",
      "rust.node.Duplex": "tsonic_rust_node::stream::Duplex",
      "rust.node.Transform": "tsonic_rust_node::stream::Transform",
      "rust.node.ReadStream": "tsonic_rust_node::fs::ReadStream",
      "rust.node.WriteStream": "tsonic_rust_node::fs::WriteStream",
      "rust.node.ReadStreamOptions": "tsonic_rust_node::fs::ReadStreamOptions",
      "rust.node.WriteStreamOptions": "tsonic_rust_node::fs::WriteStreamOptions",
      "rust.node.FsWatcher": "tsonic_rust_node::fs::FsWatcher",
      "rust.node.DnsLookupAddress": "tsonic_rust_node::dns::LookupAddress",
      "rust.node.ZlibOptions": "tsonic_rust_node::zlib::SourceZlibOptions",
      "rust.node.BrotliOptions": "tsonic_rust_node::zlib::SourceBrotliOptions",
      "rust.node.ZlibTransform": "tsonic_rust_node::zlib::Zlib",
      "rust.node.NetSocket": "tsonic_rust_node::net::Socket",
      "rust.node.NetServer": "tsonic_rust_node::net::Server",
      "rust.node.TlsConnectOptions": "tsonic_rust_node::tls::SourceConnectOptions",
      "rust.node.TlsServerOptions": "tsonic_rust_node::tls::SourceServerOptions",
      "rust.node.TlsSocket": "tsonic_rust_node::tls::TlsSocket",
      "rust.node.TlsServer": "tsonic_rust_node::tls::TlsServer",
      "rust.node.HttpsServer": "tsonic_rust_node::https::ServerHandle",
      "rust.node.HttpsClientRequest": "tsonic_rust_node::https::ClientRequest",
      "rust.node.ReadlineOptions": "tsonic_rust_node::readline::SourceInterfaceOptions",
      "rust.node.ReadlineInterface": "tsonic_rust_node::readline::Interface",
      "rust.node.Worker": "tsonic_rust_node::worker_threads::Worker",
      "rust.node.WorkerOptions": "tsonic_rust_node::worker_threads::WorkerOptions",
      "rust.node.MessagePort": "tsonic_rust_node::worker_threads::MessagePort",
      "rust.node.MessageChannel": "tsonic_rust_node::worker_threads::MessageChannel",
      "rust.node.NodeError": "tsonic_rust_node::NodeError",
    },
    carrierTraits: {
      "rust.node.Stats": cloneOnlyCarrierTraits,
      "rust.node.StatOptions": copyDefaultCarrierTraits,
      "rust.node.BufferDirectoryOptions": cloneDefaultCarrierTraits,
      "rust.node.FsConstants": { implementations: [{ traitPath: "core::clone::Clone", requirements: [] }, { traitPath: "core::marker::Copy", requirements: [] }] },
      "rust.node.Dirent": { implementations: [{ traitPath: "core::clone::Clone", requirements: [{ typeArgumentIndex: 0, traitPath: "core::clone::Clone" }] }] },
      "rust.node.MakeDirectoryOptions": copyDefaultCarrierTraits,
      "rust.node.RmOptions": copyDefaultCarrierTraits,
      "rust.node.Buffer": closedJsValueCarrierTraits,
      "rust.node.SpawnSyncResult": cloneOnlyCarrierTraits,
      "rust.node.SpawnSyncOptions": cloneDefaultCarrierTraits,
      "rust.node.Url": cloneOnlyCarrierTraits,
      "rust.node.UrlObject": cloneOnlyCarrierTraits,
      "rust.node.UrlSearchParams": cloneOnlyCarrierTraits,
      "rust.node.Hash": cloneOnlyCarrierTraits,
      "rust.node.Hmac": cloneOnlyCarrierTraits,
      "rust.node.MemoryUsage": cloneOnlyCarrierTraits,
      "rust.node.HeapInfo": cloneOnlyCarrierTraits,
      "rust.node.ProcessEnv": cloneDefaultCarrierTraits,
      "rust.node.Process": copyDefaultCarrierTraits,
      "rust.node.CpuUsage": cloneDefaultCarrierTraits,
      "rust.node.BufferEncodingOptions": cloneDefaultCarrierTraits,
      "rust.node.RealpathSync": copyDefaultCarrierTraits,
      "rust.node.Performance": copyDefaultCarrierTraits,
      "rust.node.HttpIncomingMessage": cloneOnlyCarrierTraits,
      "rust.node.HttpServerResponse": cloneOnlyCarrierTraits,
      "rust.node.HttpServer": cloneOnlyCarrierTraits,
      "rust.node.IncomingHttpHeaders": cloneOnlyCarrierTraits,
      "rust.node.OutgoingHttpHeaders": cloneOnlyCarrierTraits,
      "rust.node.HttpAddressInfo": cloneOnlyCarrierTraits,
      "rust.node.HttpServerAddress": cloneOnlyCarrierTraits,
      "rust.node.Timeout": cloneOnlyCarrierTraits,
      "rust.node.TextDecoder": cloneOnlyCarrierTraits,
      "rust.node.TextEncoder": copyDefaultCarrierTraits,
      "rust.node.Crypto": copyDefaultCarrierTraits,
      "rust.node.NodeError": cloneOnlyCarrierTraits,
      "rust.node.Stream": cloneOnlyCarrierTraits,
      "rust.node.Readable": cloneOnlyCarrierTraits,
      "rust.node.Writable": cloneOnlyCarrierTraits,
      "rust.node.Duplex": cloneOnlyCarrierTraits,
      "rust.node.Transform": cloneOnlyCarrierTraits,
      "rust.node.ReadStream": cloneOnlyCarrierTraits,
      "rust.node.WriteStream": cloneOnlyCarrierTraits,
      "rust.node.ZlibTransform": cloneOnlyCarrierTraits,
      "rust.node.NetSocket": cloneOnlyCarrierTraits,
      "rust.node.NetServer": cloneOnlyCarrierTraits,
      "rust.node.FsWatcher": cloneOnlyCarrierTraits,
      "rust.node.DnsLookupAddress": cloneOnlyCarrierTraits,
      "rust.node.ZlibOptions": cloneOnlyCarrierTraits,
      "rust.node.BrotliOptions": cloneDefaultCarrierTraits,
      "rust.node.TlsConnectOptions": cloneDefaultCarrierTraits,
      "rust.node.TlsServerOptions": cloneDefaultCarrierTraits,
      "rust.node.TlsServer": cloneOnlyCarrierTraits,
      "rust.node.HttpsServer": cloneOnlyCarrierTraits,
      "rust.node.HttpsClientRequest": cloneOnlyCarrierTraits,
      "rust.node.ReadlineOptions": cloneDefaultCarrierTraits,
      "rust.node.Worker": workerThreadCarrierTraits.worker,
      "rust.node.WorkerOptions": workerThreadCarrierTraits.options,
      "rust.node.MessagePort": workerThreadCarrierTraits.port,
    },
    binaryHooks: [{
      id: "node-performance-clock",
      phase: "before-initialization",
      path: "tsonic_rust_node::perf_hooks::initialize_clock",
      requiredCrate: "tsonic_rust_node",
    }, {
      id: "node-event-loop",
      phase: "after-entry",
      path: "tsonic_rust_node::run_event_loop",
      requiredCrate: "tsonic_rust_node",
      isFallible: true,
      errorBoundary: "source-program",
    }, {
      id: "node-process-exit-code",
      phase: "after-entry",
      path: "tsonic_rust_node::process::apply_exit_code",
      requiredCrate: "tsonic_rust_node",
    }],
    crates: [{
      crateName: "tsonic_rust_node",
      cargoPath: resolve(packageRoot, "rust/crates/tsonic_rust_node"),
      registryPatch: "crates-io",
    }],
  });
  const contributeProviderRuntime = providerPackage.runtimeContributions;
  if (contributeProviderRuntime === undefined) {
    throw new Error("The Rust Node provider package did not expose its runtime crate.");
  }
  return Object.freeze({
    ...providerPackage,
    runtimeContributions(
      context: Parameters<NonNullable<RustProviderPackageImplementation["runtimeContributions"]>>[0],
    ) {
      const ownContributions = contributeProviderRuntime(context);
      if (context.selectedSurfaceIds.includes("js")) {
        return ownContributions;
      }
      return Object.freeze({
        ...ownContributions,
        references: Object.freeze([
          ...(ownContributions.references ?? []),
          Object.freeze({
            kind: "cargo-path",
            include: resolve(rustJsPackageRoot, "crates/tsonic_rust_js"),
            attributes: Object.freeze({
              crate: "tsonic_rust_js",
              registryPatch: "crates-io",
              minimumFoundation: "std",
            }),
          }),
        ]),
      });
    },
  });
}
