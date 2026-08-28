import {
  boolCarrier,
  booleanType,
  float64Carrier,
  propertyMember,
  providerNativeFallibility,
  providerRef,
  readableCarrier,
  readlineInterfaceCarrier,
  readlineOptionsCarrier,
  readlineQuestionCallbackCarrier,
  rustOptionTargetType,
  stringCarrier,
  stringType,
  unitCarrier,
  voidType,
  writableCarrier,
} from "../model.js";
import type {
  ProviderTypeExpr,
  RustProviderModuleDefinition,
  RustProviderOperationDefinition,
  RustTargetTypeRef,
} from "../model.js";

const moduleSpecifier = "node:readline";
const optionsId = `${moduleSpecifier}::ReadLineOptions`;
const interfaceId = `${moduleSpecifier}::Interface`;
const interfaceType = providerRef(moduleSpecifier, "Interface");
const questionCallbackType: ProviderTypeExpr = {
  kind: "function",
  id: `${moduleSpecifier}.QuestionCallback`,
  parameters: [{ name: "answer", type: stringType }],
  returnType: voidType,
};

export function readlineModule(): RustProviderModuleDefinition {
  return {
    moduleSpecifier,
    providerModuleId: "tsonic.rust.node.readline",
    imports: [{
      moduleSpecifier: "node:stream",
      namedImports: [{ exportedName: "Readable" }, { exportedName: "Writable" }],
    }],
    exports: [
      {
        id: optionsId,
        name: "ReadLineOptions",
        kind: "interface",
        members: [
          propertyMember(optionsId, "input", providerRef("node:stream", "Readable"), { readonly: false }),
          propertyMember(optionsId, "output", providerRef("node:stream", "Writable"), { readonly: false, optional: true }),
          propertyMember(optionsId, "terminal", booleanType, { readonly: false, optional: true }),
          propertyMember(optionsId, "prompt", stringType, { readonly: false, optional: true }),
        ],
      },
      {
        id: interfaceId,
        name: "Interface",
        kind: "class",
        members: [
          {
            id: `${interfaceId}.question`,
            name: "question",
            kind: "method",
            signatures: [{
              id: `${interfaceId}.question(query,callback)`,
              parameters: [
                { name: "query", type: stringType },
                { name: "callback", type: questionCallbackType },
              ],
              returnType: voidType,
            }],
          },
          method("write", [{ name: "text", type: stringType }], voidType),
          method("pause", [], interfaceType),
          method("resume", [], interfaceType),
          method("isPaused", [], booleanType),
          method("close", [], voidType),
          method("setPrompt", [{ name: "prompt", type: stringType }], voidType),
          method("getPrompt", [], stringType),
          method("prompt", [], voidType),
          propertyMember(interfaceId, "line", stringType),
          propertyMember(interfaceId, "cursor", { kind: "number" }),
          propertyMember(interfaceId, "terminal", booleanType),
        ],
      },
      {
        id: `${moduleSpecifier}::createInterface`,
        name: "createInterface",
        kind: "function",
        signatures: [{
          id: `${moduleSpecifier}::createInterface(options)`,
          parameters: [{ name: "options", type: providerRef(moduleSpecifier, "ReadLineOptions") }],
          returnType: interfaceType,
        }],
      },
    ],
  };
}

export function readlineRows(): readonly RustProviderOperationDefinition[] {
  const mutableInterface: RustTargetTypeRef = {
    kind: "reference",
    referent: readlineInterfaceCarrier,
    mutable: true,
  };
  return [
    ...optionRows(),
    {
      exportId: `${moduleSpecifier}::createInterface`,
      operationKind: "method",
      target: { form: "call", path: "node_readline::create_interface", argModes: ["value"] },
      resultCarrier: readlineInterfaceCarrier,
      parameterCarriers: [readlineOptionsCarrier],
    },
    interfaceMethod("question", "question_callable", [stringCarrier, readlineQuestionCallbackCarrier], ["ref", "value"], unitCarrier, true),
    interfaceMethod("write", "write", [stringCarrier], ["ref"], unitCarrier, true),
    interfaceMethod("pause", "pause_chain", [], [], mutableInterface, false),
    interfaceMethod("resume", "resume_chain", [], [], mutableInterface, false),
    interfaceMethod("isPaused", "is_paused", [], [], boolCarrier, false),
    interfaceMethod("close", "close", [], [], unitCarrier, false),
    interfaceMethod("setPrompt", "set_prompt", [stringCarrier], ["ref"], unitCarrier, false),
    interfaceMethod("getPrompt", "get_prompt", [], [], stringCarrier, false),
    interfaceMethod("prompt", "prompt", [], [], unitCarrier, true),
    interfaceProperty("line", "line", stringCarrier),
    interfaceProperty("cursor", "cursor_number", float64Carrier),
    interfaceProperty("terminal", "terminal", boolCarrier),
  ];
}

function method(
  name: string,
  parameters: readonly { readonly name: string; readonly type: ProviderTypeExpr }[],
  returnType: ProviderTypeExpr,
) {
  return {
    id: `${interfaceId}.${name}`,
    name,
    kind: "method" as const,
    signatures: [{
      id: `${interfaceId}.${name}(${parameters.map((parameter) => parameter.name).join(",")})`,
      parameters,
      returnType,
    }],
  };
}

function optionRows(): readonly RustProviderOperationDefinition[] {
  const fields = [
    ["input", "input", readableCarrier],
    ["output", "output", rustOptionTargetType(writableCarrier)],
    ["terminal", "terminal", rustOptionTargetType(boolCarrier)],
    ["prompt", "prompt", rustOptionTargetType(stringCarrier)],
  ] as const;
  return fields.flatMap(([sourceName, targetName, carrier]) => [
    {
      exportId: optionsId,
      memberId: `${optionsId}.${sourceName}`,
      operationKind: "property" as const,
      target: { form: "field" as const, name: targetName },
      resultCarrier: carrier,
      receiverCarrier: readlineOptionsCarrier,
    },
    {
      exportId: optionsId,
      memberId: `${optionsId}.${sourceName}`,
      operationKind: "property-set" as const,
      target: { form: "field" as const, name: targetName },
      resultCarrier: unitCarrier,
      receiverCarrier: readlineOptionsCarrier,
      parameterCarriers: [carrier],
    },
  ]);
}

function interfaceMethod(
  member: string,
  name: string,
  parameters: readonly RustTargetTypeRef[],
  argModes: readonly ("value" | "ref" | "mut-ref")[],
  resultCarrier: RustTargetTypeRef,
  fallible: boolean,
): RustProviderOperationDefinition {
  return {
    exportId: interfaceId,
    memberId: `${interfaceId}.${member}`,
    operationKind: "method",
    target: {
      form: "receiver-method",
      name,
      mutatesReceiver: true,
      ...(argModes.length === 0 ? {} : { argModes }),
    },
    resultCarrier,
    receiverCarrier: readlineInterfaceCarrier,
    parameterCarriers: parameters,
    ...(fallible ? providerNativeFallibility : {}),
  };
}

function interfaceProperty(
  member: string,
  name: string,
  resultCarrier: RustTargetTypeRef,
): RustProviderOperationDefinition {
  return {
    exportId: interfaceId,
    memberId: `${interfaceId}.${member}`,
    operationKind: "property",
    target: { form: "receiver-method", name },
    resultCarrier,
    receiverCarrier: readlineInterfaceCarrier,
  };
}
