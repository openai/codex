import { sanitizeToolName } from "./sanitizeToolName";
import { type Tool as MCPTool } from "@modelcontextprotocol/sdk/types.js";
import { type FunctionTool as OpenAIFunctionTool } from "openai/resources/responses/responses.mjs";

function removeUnsupportedKeysFromJsonSchemaParameters(
  parameters: Record<string, unknown>,
  keys: Array<string>,
): Record<string, unknown> {
  // Create a deep copy of the parameters to avoid modifying the original
  const paramsCopy = JSON.parse(JSON.stringify(parameters));

  // Remove specified keys from the top level
  for (const key of keys) {
    delete paramsCopy[key];
  }

  // If there are properties, recursively process them
  if (paramsCopy.properties && typeof paramsCopy.properties === "object") {
    for (const propName in paramsCopy.properties) {
      if (
        Object.prototype.hasOwnProperty.call(paramsCopy.properties, propName)
      ) {
        const property = paramsCopy.properties[propName];
        if (property && typeof property === "object") {
          // Remove specified keys from each property
          for (const key of keys) {
            delete property[key];
          }
        }
      }
    }
  }

  return paramsCopy;
}

// Recursively remove unsupported keys from JSON schema
function removeUnsupportedKeysFromJsonSchemaParametersRecursive(
  parameters: Record<string, unknown>,
): Record<string, unknown> {
  return removeUnsupportedKeysFromJsonSchemaParameters(parameters, ["default"]);
}

/* 
  Error: 400 Invalid schema for function 'xx': In context=(), 'additionalProperties' is required to be supplied and to be false.
  // Recursively set "additionalProperties" to false, where "type" is "array, "object" or "string"
*/

// Recursively set additionalProperties: false for all object schemas
function setAdditionalPropertiesFalse(
  schema: Record<string, unknown>,
): Record<string, unknown> {
  const schemaCopy = JSON.parse(JSON.stringify(schema));

  // Set additionalProperties: false for the current schema if it's an object type
  if (schemaCopy.type === "object" || !schemaCopy.type) {
    schemaCopy.additionalProperties = false;
  }

  // Process properties recursively
  if (schemaCopy.properties && typeof schemaCopy.properties === "object") {
    for (const propName in schemaCopy.properties) {
      if (
        Object.prototype.hasOwnProperty.call(schemaCopy.properties, propName)
      ) {
        const property = schemaCopy.properties[propName];
        if (property && typeof property === "object") {
          schemaCopy.properties[propName] = setAdditionalPropertiesFalse(
            property as Record<string, unknown>,
          );
        }
      }
    }
  }

  // Handle items in arrays
  if (
    schemaCopy.type === "array" &&
    schemaCopy.items &&
    typeof schemaCopy.items === "object"
  ) {
    schemaCopy.items = setAdditionalPropertiesFalse(
      schemaCopy.items as Record<string, unknown>,
    );
  }

  return schemaCopy;
}

function ensureBaseSchema(
  parameters: Record<string, unknown>,
): Record<string, unknown> {
  return {
    ...parameters,
    required: Object.keys(parameters["properties"] || {}),
    $schema: "http://json-schema.org/draft-07/schema#",
    additionalProperties: false,
  };
}

// function piping
function pipe<T>(...fns: Array<(arg: T) => T>): (arg: T) => T {
  return (arg: T): T => {
    return fns.reduce((acc, fn) => fn(acc), arg);
  };
}

export function mcpToOpenaiTools(
  tools: Array<MCPTool>,
): Array<OpenAIFunctionTool> {
  return tools.map((tool: MCPTool): OpenAIFunctionTool => {
    const inputSchema = pipe(
      removeUnsupportedKeysFromJsonSchemaParametersRecursive,
      setAdditionalPropertiesFalse,
      ensureBaseSchema,
    )(tool.inputSchema);

    // Sanitize the tool name to ensure it complies with OpenAI's pattern
    const sanitizedName = sanitizeToolName(tool.name);

    return {
      type: "function",
      name: sanitizedName,
      parameters: inputSchema,
      strict: true,
      description: tool.description,
    };
  });
}
