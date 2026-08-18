import {
  boolCarrier,
  booleanType,
  bufferCarrier,
  float64Carrier,
  fnExport,
  int32Carrier,
  methodMember,
  noneArgument,
  numberType,
  propertyMember,
  providerNativeFallibility,
  providerRef,
  rustInt32ToUsizeValueConversion,
  rustJsArrayTargetType,
  rustUsizeToInt32ValueConversion,
  stringCarrier,
  stringType,
  zeroFloat64Argument,
} from "../model.js";

import type {
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
} from "../model.js";
interface BufferNumericMemberDefinition {
  readonly sourceName: string;
  readonly targetName: string;
  readonly mode: "read" | "write";
}

const bufferNumericMembers: readonly BufferNumericMemberDefinition[] = Object.freeze([
  { sourceName: "readUInt8", targetName: "read_uint8_number", mode: "read" },
  { sourceName: "readInt8", targetName: "read_int8_number", mode: "read" },
  { sourceName: "readUInt16LE", targetName: "read_uint16_le_number", mode: "read" },
  { sourceName: "readUInt16BE", targetName: "read_uint16_be_number", mode: "read" },
  { sourceName: "readInt16LE", targetName: "read_int16_le_number", mode: "read" },
  { sourceName: "readInt16BE", targetName: "read_int16_be_number", mode: "read" },
  { sourceName: "readUInt32LE", targetName: "read_uint32_le_number", mode: "read" },
  { sourceName: "readUInt32BE", targetName: "read_uint32_be_number", mode: "read" },
  { sourceName: "readInt32LE", targetName: "read_int32_le_number", mode: "read" },
  { sourceName: "readInt32BE", targetName: "read_int32_be_number", mode: "read" },
  { sourceName: "readFloatLE", targetName: "read_float_le_number", mode: "read" },
  { sourceName: "readFloatBE", targetName: "read_float_be_number", mode: "read" },
  { sourceName: "readDoubleLE", targetName: "read_double_le_number", mode: "read" },
  { sourceName: "readDoubleBE", targetName: "read_double_be_number", mode: "read" },
  { sourceName: "writeUInt8", targetName: "write_uint8_number", mode: "write" },
  { sourceName: "writeInt8", targetName: "write_int8_number", mode: "write" },
  { sourceName: "writeUInt16LE", targetName: "write_uint16_le_number", mode: "write" },
  { sourceName: "writeUInt16BE", targetName: "write_uint16_be_number", mode: "write" },
  { sourceName: "writeInt16LE", targetName: "write_int16_le_number", mode: "write" },
  { sourceName: "writeInt16BE", targetName: "write_int16_be_number", mode: "write" },
  { sourceName: "writeUInt32LE", targetName: "write_uint32_le_number", mode: "write" },
  { sourceName: "writeUInt32BE", targetName: "write_uint32_be_number", mode: "write" },
  { sourceName: "writeInt32LE", targetName: "write_int32_le_number", mode: "write" },
  { sourceName: "writeInt32BE", targetName: "write_int32_be_number", mode: "write" },
  { sourceName: "writeFloatLE", targetName: "write_float_le_number", mode: "write" },
  { sourceName: "writeFloatBE", targetName: "write_float_be_number", mode: "write" },
  { sourceName: "writeDoubleLE", targetName: "write_double_le_number", mode: "write" },
  { sourceName: "writeDoubleBE", targetName: "write_double_be_number", mode: "write" },
]);

function bufferNumericMemberDeclarations(bufferId: string) {
  return bufferNumericMembers.map((member) => {
    const memberId = `${bufferId}.${member.sourceName}`;
    const valueParameters = member.mode === "read"
      ? []
      : [{ name: "value", type: numberType }];
    return {
      id: memberId,
      name: member.sourceName,
      kind: "method" as const,
      signatures: [
        {
          id: `${memberId}(${valueParameters.map((parameter) => parameter.name).join(",")})`,
          parameters: valueParameters,
          returnType: numberType,
        },
        {
          id: `${memberId}(${[...valueParameters.map((parameter) => parameter.name), "offset"].join(",")})`,
          parameters: [...valueParameters, { name: "offset", type: numberType }],
          returnType: numberType,
        },
      ],
    };
  });
}

function bufferNumericRows(bufferId: string): readonly RustProviderOperationDefinition[] {
  return bufferNumericMembers.flatMap((member): readonly RustProviderOperationDefinition[] => {
    const memberId = `${bufferId}.${member.sourceName}`;
    const valueCarriers = member.mode === "read" ? [] : [float64Carrier];
    const target = {
      form: "free-call" as const,
      path: `node_buffer::${member.targetName}`,
      receiverMode: member.mode === "read" ? "ref" as const : "mut-ref" as const,
    } as const;
    return [{
      exportId: bufferId,
      memberId,
      signatureId: `${memberId}(${member.mode === "read" ? "" : "value"})`,
      operationKind: "method",
      target: { ...target, trailingArguments: [zeroFloat64Argument] },
      resultCarrier: float64Carrier,
      parameterCarriers: valueCarriers,
      ...providerNativeFallibility,
    }, {
      exportId: bufferId,
      memberId,
      signatureId: `${memberId}(${member.mode === "read" ? "offset" : "value,offset"})`,
      operationKind: "method",
      target,
      resultCarrier: float64Carrier,
      parameterCarriers: [...valueCarriers, float64Carrier],
      ...providerNativeFallibility,
    }];
  });
}

export function bufferModule(): RustProviderModuleDefinition {
  const m = "node:buffer";
  const bufferId = "node:buffer::Buffer";
  return {
    moduleSpecifier: m,
    providerModuleId: "tsonic.rust.node.buffer",
    exports: [
      {
        id: bufferId,
        name: "Buffer",
        kind: "class" as const,
        members: [
          {
            id: `${bufferId}.from`,
            name: "from",
            kind: "method" as const,
            static: true,
            signatures: [
              {
                id: `${bufferId}.from(string)`,
                parameters: [{ name: "value", type: stringType }],
                returnType: providerRef(m, "Buffer"),
              },
              {
                id: `${bufferId}.from(string,encoding)`,
                parameters: [{ name: "value", type: stringType }, { name: "encoding", type: stringType }],
                returnType: providerRef(m, "Buffer"),
              },
              {
                id: `${bufferId}.from(numberArray)`,
                parameters: [{ name: "value", type: { kind: "array", elementType: numberType } }],
                returnType: providerRef(m, "Buffer"),
              },
            ],
          },
          methodMember(bufferId, "alloc", [{ name: "size", type: numberType }], providerRef(m, "Buffer"), { static: true }),
          methodMember(bufferId, "byteLength", [{ name: "value", type: stringType }, { name: "encoding", type: stringType }], numberType, { static: true }),
          methodMember(bufferId, "concat", [{ name: "list", type: { kind: "array", elementType: providerRef(m, "Buffer") } }], providerRef(m, "Buffer"), { static: true }),
          methodMember(bufferId, "toString", [{ name: "encoding", type: stringType }], stringType),
          {
            id: `${bufferId}.copy`,
            name: "copy",
            kind: "method",
            signatures: [
              { id: `${bufferId}.copy(target)`, parameters: [{ name: "target", type: providerRef(m, "Buffer") }], returnType: numberType },
              { id: `${bufferId}.copy(target,targetStart)`, parameters: [{ name: "target", type: providerRef(m, "Buffer") }, { name: "targetStart", type: numberType }], returnType: numberType },
              { id: `${bufferId}.copy(target,targetStart,sourceStart)`, parameters: [{ name: "target", type: providerRef(m, "Buffer") }, { name: "targetStart", type: numberType }, { name: "sourceStart", type: numberType }], returnType: numberType },
              { id: `${bufferId}.copy(target,targetStart,sourceStart,sourceEnd)`, parameters: [{ name: "target", type: providerRef(m, "Buffer") }, { name: "targetStart", type: numberType }, { name: "sourceStart", type: numberType }, { name: "sourceEnd", type: numberType }], returnType: numberType },
            ],
          },
          ...["slice", "subarray"].map((name) => ({
            id: `${bufferId}.${name}`,
            name,
            kind: "method" as const,
            signatures: [
              { id: `${bufferId}.${name}()`, parameters: [], returnType: providerRef(m, "Buffer") },
              { id: `${bufferId}.${name}(start)`, parameters: [{ name: "start", type: numberType }], returnType: providerRef(m, "Buffer") },
              { id: `${bufferId}.${name}(start,end)`, parameters: [{ name: "start", type: numberType }, { name: "end", type: numberType }], returnType: providerRef(m, "Buffer") },
            ],
          })),
          ...["swap16", "swap32", "swap64"].map((name) => methodMember(bufferId, name, [], providerRef(m, "Buffer"))),
          ...bufferNumericMemberDeclarations(bufferId),
          methodMember(bufferId, "equals", [{ name: "other", type: providerRef(m, "Buffer") }], booleanType),
          methodMember(bufferId, "compare", [{ name: "other", type: providerRef(m, "Buffer") }], numberType),
          propertyMember(bufferId, "length", numberType),
        ],
      },
      fnExport(m, "isBuffer", [{ name: "value", type: providerRef(m, "Buffer") }], booleanType),
      fnExport(m, "btoa", [{ name: "value", type: stringType }], stringType),
      fnExport(m, "atob", [{ name: "value", type: stringType }], stringType),
      fnExport(m, "isEncoding", [{ name: "encoding", type: stringType }], booleanType),
    ],
  };
}

export function bufferRows(): readonly RustProviderOperationDefinition[] {
  const bufferId = "node:buffer::Buffer";
  return [
    { exportId: bufferId, memberId: `${bufferId}.from`, signatureId: `${bufferId}.from(string)`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::from_string", argModes: ["ref"], trailingArguments: [noneArgument] }, resultCarrier: bufferCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: bufferId, memberId: `${bufferId}.from`, signatureId: `${bufferId}.from(string,encoding)`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::from_string_enc", argModes: ["ref", "ref"] }, resultCarrier: bufferCarrier, parameterCarriers: [stringCarrier, stringCarrier], ...providerNativeFallibility },
    { exportId: bufferId, memberId: `${bufferId}.from`, signatureId: `${bufferId}.from(numberArray)`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::from_number_array", argModes: ["ref"] }, resultCarrier: bufferCarrier, parameterCarriers: [rustJsArrayTargetType(float64Carrier)] },
    { exportId: bufferId, memberId: `${bufferId}.alloc`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::alloc", argConversions: [rustInt32ToUsizeValueConversion] }, resultCarrier: bufferCarrier, parameterCarriers: [int32Carrier] },
    { exportId: bufferId, memberId: `${bufferId}.byteLength`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::byte_length_enc", argModes: ["ref", "ref"] }, resultCarrier: int32Carrier, parameterCarriers: [stringCarrier, stringCarrier], ...providerNativeFallibility, resultConversion: rustUsizeToInt32ValueConversion },
    { exportId: bufferId, memberId: `${bufferId}.concat`, operationKind: "method", target: { form: "call", path: "node_buffer::Buffer::concat", argModes: ["ref"] }, resultCarrier: bufferCarrier, parameterCarriers: [rustJsArrayTargetType(bufferCarrier)], ...providerNativeFallibility },
    { exportId: bufferId, memberId: `${bufferId}.toString`, operationKind: "method", target: { form: "receiver-method", name: "to_string_enc", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    {
      exportId: bufferId,
      memberId: `${bufferId}.copy`,
      signatureId: `${bufferId}.copy(target)`,
      operationKind: "method",
      target: { form: "free-call", path: "node_buffer::copy_open_number", receiverMode: "ref", argModes: ["ref"], trailingArguments: [zeroFloat64Argument, zeroFloat64Argument] },
      resultCarrier: float64Carrier,
      parameterCarriers: [bufferCarrier],
      ...providerNativeFallibility,
    },
    {
      exportId: bufferId,
      memberId: `${bufferId}.copy`,
      signatureId: `${bufferId}.copy(target,targetStart)`,
      operationKind: "method",
      target: { form: "free-call", path: "node_buffer::copy_open_number", receiverMode: "ref", argModes: ["ref", "value"], trailingArguments: [zeroFloat64Argument] },
      resultCarrier: float64Carrier,
      parameterCarriers: [bufferCarrier, float64Carrier],
      ...providerNativeFallibility,
    },
    {
      exportId: bufferId,
      memberId: `${bufferId}.copy`,
      signatureId: `${bufferId}.copy(target,targetStart,sourceStart)`,
      operationKind: "method",
      target: { form: "free-call", path: "node_buffer::copy_open_number", receiverMode: "ref", argModes: ["ref", "value", "value"] },
      resultCarrier: float64Carrier,
      parameterCarriers: [bufferCarrier, float64Carrier, float64Carrier],
      ...providerNativeFallibility,
    },
    {
      exportId: bufferId,
      memberId: `${bufferId}.copy`,
      signatureId: `${bufferId}.copy(target,targetStart,sourceStart,sourceEnd)`,
      operationKind: "method",
      target: { form: "free-call", path: "node_buffer::copy_closed_number", receiverMode: "ref", argModes: ["ref", "value", "value", "value"] },
      resultCarrier: float64Carrier,
      parameterCarriers: [bufferCarrier, float64Carrier, float64Carrier, float64Carrier],
      ...providerNativeFallibility,
    },
    ...["slice", "subarray"].flatMap((name): readonly RustProviderOperationDefinition[] => [{
      exportId: bufferId,
      memberId: `${bufferId}.${name}`,
      signatureId: `${bufferId}.${name}()`,
      operationKind: "method",
      target: { form: "free-call", path: "node_buffer::slice_open_number", receiverMode: "ref", trailingArguments: [zeroFloat64Argument] },
      resultCarrier: bufferCarrier,
      parameterCarriers: [],
    }, {
      exportId: bufferId,
      memberId: `${bufferId}.${name}`,
      signatureId: `${bufferId}.${name}(start)`,
      operationKind: "method",
      target: { form: "free-call", path: "node_buffer::slice_open_number", receiverMode: "ref" },
      resultCarrier: bufferCarrier,
      parameterCarriers: [float64Carrier],
    }, {
      exportId: bufferId,
      memberId: `${bufferId}.${name}`,
      signatureId: `${bufferId}.${name}(start,end)`,
      operationKind: "method",
      target: { form: "free-call", path: "node_buffer::slice_closed_number", receiverMode: "ref" },
      resultCarrier: bufferCarrier,
      parameterCarriers: [float64Carrier, float64Carrier],
    }]),
    ...["swap16", "swap32", "swap64"].map((name): RustProviderOperationDefinition => ({
      exportId: bufferId,
      memberId: `${bufferId}.${name}`,
      signatureId: `${bufferId}.${name}()`,
      operationKind: "method",
      target: { form: "receiver-method", name, mutatesReceiver: true },
      resultCarrier: bufferCarrier,
      parameterCarriers: [],
      ...providerNativeFallibility,
    })),
    ...bufferNumericRows(bufferId),
    { exportId: bufferId, memberId: `${bufferId}.equals`, operationKind: "method", target: { form: "receiver-method", name: "equals", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [bufferCarrier] },
    { exportId: bufferId, memberId: `${bufferId}.compare`, operationKind: "method", target: { form: "receiver-method", name: "compare", argModes: ["ref"] }, resultCarrier: int32Carrier, parameterCarriers: [bufferCarrier] },
    { exportId: bufferId, memberId: `${bufferId}.length`, operationKind: "property", target: { form: "receiver-method", name: "len" }, resultCarrier: int32Carrier, resultConversion: rustUsizeToInt32ValueConversion },
    { exportId: "node:buffer::btoa", operationKind: "method", target: { form: "call", path: "node_buffer::btoa", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: "node:buffer::atob", operationKind: "method", target: { form: "call", path: "node_buffer::atob", argModes: ["ref"] }, resultCarrier: stringCarrier, parameterCarriers: [stringCarrier], ...providerNativeFallibility },
    { exportId: "node:buffer::isEncoding", operationKind: "method", target: { form: "call", path: "node_buffer::is_encoding", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [stringCarrier] },
    { exportId: "node:buffer::isBuffer", operationKind: "method", target: { form: "call", path: "node_buffer::is_buffer", argModes: ["ref"] }, resultCarrier: boolCarrier, parameterCarriers: [bufferCarrier] },
  ];
}

// --- node:url ----------------------------------------------------------------
