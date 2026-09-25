import { fnExport, constructorMember, propertyMember, providerRef } from "../../declarations/builders.js";
import { bufferCarrier } from "../buffer/carriers.js";
import { boolCarrier, int32Carrier, jsValueCarrier, stringCarrier } from "../../model/carriers.js";
import { booleanType, numberType, stringType } from "../../model/source-types.js";
import { providerNativeFallibility } from "../../model/operations.js";
import { rustBorrowedStrToStringValueConversion } from "@tsonic/target-rust/provider";
import { textDecoderCarrier, textEncoderCarrier } from "./carriers.js";
import { rustJsTypedArrayTargetType } from "@tsonic/target-rust/provider";

import type { RustProviderModuleDefinition, RustProviderOperationDefinition } from "@tsonic/target-rust/provider";
export function utilModule(typedArrays: boolean): RustProviderModuleDefinition {
  const m = "node:util";
  const textDecoderId = `${m}::TextDecoder`;
  const textEncoderId = `${m}::TextEncoder`;
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.util",
    imports: [{
      moduleSpecifier: "node:buffer",
      namedImports: [{ exportedName: "Buffer" }],
    }],
    exports: [
      ...(typedArrays ? [{
        id: textEncoderId,
        name: "TextEncoder",
        kind: "class",
        members: [
          constructorMember(textEncoderId, []),
          {
            id: `${textEncoderId}.encode`, name: "encode", kind: "method",
            signatures: [{
              id: `${textEncoderId}.encode(input)`,
              parameters: [{ name: "input", type: stringType }],
              returnType: { kind: "source-global", name: "Uint8Array" },
            }],
          },
          propertyMember(textEncoderId, "encoding", stringType),
        ],
      } satisfies RustProviderModuleDefinition["exports"][number]] : []),
      {
        id: textDecoderId,
        name: "TextDecoder",
        kind: "class" as const,
        members: [
          constructorMember(textDecoderId, []),
          {
            id: `${textDecoderId}.decode`,
            name: "decode",
            kind: "method" as const,
            signatures: [{
              id: `${textDecoderId}.decode(input)`,
              parameters: [{ name: "input", type: providerRef("node:buffer", "Buffer") }],
              returnType: stringType,
            }, ...(typedArrays ? [{
              id: `${textDecoderId}.decode(bytes)`,
              parameters: [{ name: "input", type: { kind: "source-global" as const, name: "Uint8Array" } }],
              returnType: stringType,
            }] : [])],
          },
          propertyMember(textDecoderId, "encoding", stringType),
          propertyMember(textDecoderId, "fatal", booleanType),
          propertyMember(textDecoderId, "ignoreBOM", booleanType),
        ],
      },
      fnExport(m, "stripVTControlCharacters", [{ name: "value", type: stringType }], stringType),
      fnExport(m, "toUSVString", [{ name: "value", type: stringType }], stringType),
      fnExport(m, "styleText", [{ name: "style", type: stringType }, { name: "text", type: stringType }], stringType),
      fnExport(m, "getSystemErrorName", [{ name: "code", type: numberType }], stringType),
      fnExport(m, "getSystemErrorMessage", [{ name: "code", type: numberType }], stringType),
      fnExport(m, "inspect", [{ name: "value", type: { kind: "any" } }], stringType),
      fnExport(m, "format", [{ name: "format", type: stringType }, { name: "values", type: { kind: "array", elementType: { kind: "any" } }, rest: true }], stringType),
    ],
  };
}

export function utilRows(typedArrays: boolean): readonly RustProviderOperationDefinition[] {
  const m = "node:util";
  const textDecoderId = `${m}::TextDecoder`;
  const textEncoderId = `${m}::TextEncoder`;
  return [
    ...(typedArrays ? [
    { exportId: textEncoderId, memberId: `${textEncoderId}.constructor`, signatureId: `${textEncoderId}.constructor()`, operationKind: "constructor", target: { form: "call", path: "node_util::TextEncoder::new" }, resultCarrier: textEncoderCarrier, parameterCarriers: [] },
    { exportId: textEncoderId, memberId: `${textEncoderId}.encode`, signatureId: `${textEncoderId}.encode(input)`, operationKind: "method", target: { form: "receiver-method", name: "encode_uint8", argModes: ["ref"] }, resultCarrier: rustJsTypedArrayTargetType("Uint8Array"), parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: textEncoderId, memberId: `${textEncoderId}.encoding`, operationKind: "property", target: { form: "receiver-method", name: "encoding" }, resultCarrier: stringCarrier, resultConversion: rustBorrowedStrToStringValueConversion },
    { exportId: textDecoderId, memberId: `${textDecoderId}.decode`, signatureId: `${textDecoderId}.decode(bytes)`, operationKind: "method", target: { form: "receiver-method", name: "decode_uint8", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [rustJsTypedArrayTargetType("Uint8Array")], ...providerNativeFallibility },
    ] satisfies RustProviderOperationDefinition[] : []),
    { exportId: textDecoderId, memberId: `${textDecoderId}.constructor`, signatureId: `${textDecoderId}.constructor()`, operationKind: "constructor", target: { form: "call", path: "node_util::text_decoder_new" }, resultCarrier: textDecoderCarrier, parameterCarriers: [] },
    { exportId: textDecoderId, memberId: `${textDecoderId}.decode`, signatureId: `${textDecoderId}.decode(input)`, operationKind: "method", target: { form: "receiver-method", name: "decode_buffer", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [bufferCarrier], ...providerNativeFallibility },
    { exportId: textDecoderId, memberId: `${textDecoderId}.encoding`, operationKind: "property", target: { form: "receiver-method", name: "encoding" }, resultCarrier: stringCarrier, resultConversion: rustBorrowedStrToStringValueConversion },
    { exportId: textDecoderId, memberId: `${textDecoderId}.fatal`, operationKind: "property", target: { form: "receiver-method", name: "fatal" }, resultCarrier: boolCarrier },
    { exportId: textDecoderId, memberId: `${textDecoderId}.ignoreBOM`, operationKind: "property", target: { form: "receiver-method", name: "ignore_bom" }, resultCarrier: boolCarrier },
    { exportId: `${m}::stripVTControlCharacters`, operationKind: "method", target: { form: "call", path: "node_util::strip_vt_control_characters", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier] },
    { exportId: `${m}::toUSVString`, operationKind: "method", target: { form: "call", path: "node_util::to_usv_string", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier] },
    { exportId: `${m}::styleText`, operationKind: "method", target: { form: "call", path: "node_util::style_text", argModes: ["ref", "ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier, stringCarrier] },
    { exportId: `${m}::getSystemErrorName`, operationKind: "method", target: { form: "call", path: "node_util::get_system_error_name" }, resultCarrier: stringCarrier, resultConversion: rustBorrowedStrToStringValueConversion, parameterCarriers: [int32Carrier] },
    { exportId: `${m}::inspect`, operationKind: "method", target: { form: "call", path: "node_util::inspect", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [jsValueCarrier] },
    { exportId: `${m}::getSystemErrorMessage`, operationKind: "method", target: { form: "call", path: "node_util::get_system_error_message" }, resultCarrier: stringCarrier, resultConversion: rustBorrowedStrToStringValueConversion, parameterCarriers: [int32Carrier] },
    { exportId: `${m}::format`, operationKind: "method", target: { form: "call-value-slice", path: "node_util::format", leadingArguments: [{ carrier: stringCarrier, mode: "ref" }], elementCarrier: jsValueCarrier }, resultCarrier: stringCarrier, ...providerNativeFallibility },
  ];
}
