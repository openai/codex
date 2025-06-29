import { sanitizeToolName, SERVER_TOOL_DELIMITER } from "./sanitizeToolName";
import { type MCPClient } from "../core/client";
import { type Tool } from "@modelcontextprotocol/sdk/types.js";

export async function flattenTools(
  mcpClients: Array<MCPClient>,
): Promise<Array<Tool>> {
  const flattenedTools: Array<Tool> = [];
  const toolPromises = mcpClients.map(async (mcpClient) => {
    const tools = await mcpClient.getTools();
    const updatedTools = tools.map((tool) => {
      return {
        ...tool,
        name: sanitizeToolName(
          `${mcpClient.name}${SERVER_TOOL_DELIMITER}${tool.name}`,
        ),
        description: tool.description,
      };
    });
    return updatedTools;
  });
  const tools = await Promise.all(toolPromises);
  for (const tool of tools) {
    flattenedTools.push(...tool);
  }
  return flattenedTools;
}
