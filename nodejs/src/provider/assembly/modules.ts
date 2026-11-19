import type { RustProviderModuleDefinition } from "@tsonic/target-rust/provider";
import { assertModule } from "../modules/assert.js";
import { bufferModule } from "../modules/buffer/declarations.js";
import { childProcessModule } from "../modules/child-process/declarations.js";
import { cryptoModule } from "../modules/crypto/declarations.js";
import { dnsModule, dnsPromisesModule } from "../modules/dns/declarations.js";
import { eventsModule } from "../modules/events/declarations.js";
import { fsModule } from "../modules/filesystem/calls.js";
import { fsPromisesModule } from "../modules/filesystem/promises.js";
import { httpModule } from "../modules/http/declarations.js";
import { httpsModule } from "../modules/https/declarations.js";
import { netModule } from "../modules/net/declarations.js";
import { osModule } from "../modules/os.js";
import { pathModule } from "../modules/path.js";
import { performanceModule } from "../modules/performance.js";
import { processModule } from "../modules/process/declarations.js";
import { readlineModule } from "../modules/readline/declarations.js";
import { streamModule } from "../modules/stream/declarations.js";
import { timersModule } from "../modules/timers/declarations.js";
import { tlsModule } from "../modules/tls/declarations.js";
import { urlModule } from "../modules/url/declarations.js";
import { utilModule } from "../modules/util/declarations.js";
import { v8Module } from "../modules/v8.js";
import { workerThreadsModule } from "../modules/worker-threads/declarations.js";
import { zlibModule } from "../modules/zlib/declarations.js";

export function nodejsProviderModules(typedArrays: boolean): readonly RustProviderModuleDefinition[] {
  return [
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
  ];
}
