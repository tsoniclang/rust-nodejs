import { rustOptionTargetType } from "@tsonic/target-rust/provider";
import type { RustProviderModuleDefinition, RustProviderOperationDefinition } from "@tsonic/target-rust/provider";
import { providerRef } from "../../declarations/builders.js";
import {
  httpRequestCallbackCarrier,
  httpServerCarrier,
  incomingHttpHeadersCarrier,
  outgoingHttpHeadersCarrier,
} from "./carriers.js";
import {
  moduleSpecifier,
  incomingHeadersId,
  outgoingHeadersId,
  callbackType,
} from "./types.js";
import {
  headerDeclaration,
  headerValuesDeclaration,
  headerRows,
  headerValuesRow,
} from "./headers.js";
import { incomingMessageDeclaration, incomingRows } from "./requests.js";
import { serverResponseDeclaration, responseRows } from "./responses.js";
import {
  serverDeclaration,
  addressInfoDeclaration,
  serverAddressDeclaration,
  addressRows,
  serverRows,
} from "./servers.js";

export function httpModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.http",
    imports: [
      { moduleSpecifier: "node:util", namedImports: [{ exportedName: "NodeError" }] },
      { moduleSpecifier: "node:buffer", namedImports: [{ exportedName: "Buffer" }] },
      { moduleSpecifier: "node:net", namedImports: [{ exportedName: "Socket" }] },
      { moduleSpecifier: "node:stream", namedImports: [{ exportedName: "Readable" }, { exportedName: "Writable" }] },
    ],
    exports: [
      headerDeclaration(incomingHeadersId, "IncomingHttpHeaders"),
      headerValuesDeclaration(),
      headerDeclaration(outgoingHeadersId, "OutgoingHttpHeaders"),
      addressInfoDeclaration(),
      serverAddressDeclaration(),
      incomingMessageDeclaration(),
      serverResponseDeclaration(),
      serverDeclaration(),
      {
        id: `${moduleSpecifier}::createServer`,
        name: "createServer",
        kind: "function",
        signatures: [{
          id: `${moduleSpecifier}::createServer(listener)`,
          parameters: [{
            name: "listener",
            type: callbackType(`${moduleSpecifier}::createServer(listener)`, [
              { name: "request", type: providerRef(moduleSpecifier, "IncomingMessage") },
              { name: "response", type: providerRef(moduleSpecifier, "ServerResponse") },
            ]),
            optional: true,
          }],
          returnType: providerRef(moduleSpecifier, "Server"),
        }],
      },
    ],
  };
}

export function httpRows(): readonly RustProviderOperationDefinition[] {
  return [
    {
      exportId: `${moduleSpecifier}::createServer`,
      signatureId: `${moduleSpecifier}::createServer(listener)`,
      operationKind: "method",
      target: { form: "call", path: "node_http::create_server_optional", argModes: ["value"] },
      resultCarrier: httpServerCarrier,
      parameterCarriers: [rustOptionTargetType(httpRequestCallbackCarrier)],
    },
    ...headerRows(incomingHeadersId, incomingHttpHeadersCarrier),
    headerValuesRow(),
    ...headerRows(outgoingHeadersId, outgoingHttpHeadersCarrier),
    ...addressRows(),
    ...incomingRows(),
    ...responseRows(),
    ...serverRows(),
  ];
}
