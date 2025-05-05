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

export function mcpToOpenaiTools(
  tools: Array<MCPTool>,
): Array<OpenAIFunctionTool> {
  return tools.map((tool: MCPTool): OpenAIFunctionTool => {
    return {
      type: "function",
      name: tool.name,
      parameters: removeUnsupportedKeysFromJsonSchemaParametersRecursive(
        tool.inputSchema,
      ),
      strict: true,
      description: tool.description,
    };
  });
}
