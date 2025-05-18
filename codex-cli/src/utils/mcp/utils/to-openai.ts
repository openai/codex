import { sanitizeToolName } from "./sanitizeToolName";
import { type Tool as MCPTool } from "@modelcontextprotocol/sdk/types.js";
import { type FunctionTool as OpenAIFunctionTool } from "openai/resources/responses/responses.mjs";
import { log } from "../../logger/log";

// Create a minimal valid schema that will pass OpenAI validation
function createMinimalValidSchema(): Record<string, unknown> {
  return {
    type: "object",
    additionalProperties: false,
    properties: {},
    required: []
  };
}

export function mcpToOpenaiTools(
  tools: Array<MCPTool>,
): Array<OpenAIFunctionTool> {
  return tools.map((tool: MCPTool): OpenAIFunctionTool => {
    // Sanitize the tool name to ensure it complies with OpenAI's pattern
    const sanitizedName = sanitizeToolName(tool.name);
    
    // Use a minimal valid schema for all tools to avoid validation issues
    const schema = createMinimalValidSchema();
    
    return {
      type: "function",
      name: sanitizedName,
      parameters: schema,
      strict: false, // Set strict to false to allow any parameters
      description: tool.description || "",
    };
  });
}