import { log } from "./log";
import { McpManager } from "../mcp-manager"; // Import McpManager class

/**
 * Tool call parsed from model response
 */
export interface ToolCall {
  name: string;
  args: Record<string, any>;
  toolCallText: string; // Original text of the tool call
}

/**
 * Result of a tool execution
 */
export interface ToolCallResult {
  result?: any;
  error?: string;
  toolCall: ToolCall;
}

/**
 * Parse a tool call from a model response
 */
export function parseToolCall(response: string): ToolCall | null {
  // Match tool call syntax: <tool>\nname: tool_name\nargs: {...}\n</tool>
  const toolCallMatch = response.match(/<tool>[\s\S]*?<\/tool>/);
  if (!toolCallMatch) {
    return null;
  }

  const toolCallText = toolCallMatch[0];
  // Extract content between <tool> and </tool> tags
  const contentMatch = toolCallText.match(/<tool>([\s\S]*?)<\/tool>/);
  const toolCallContent = contentMatch ? contentMatch[1] : "";

  log(`Processing tool call: ${toolCallText}`);

  // Extract name and args with more flexible pattern matching
  const nameMatch = toolCallContent?.match(/name:\s*([^\n]+)/); // Optional chaining

  // Improved args matching to handle multiline JSON
  // This searches for the pattern "args: {" followed by any content until the closing brace
  // We look for the last closing brace to handle nested objects
  const argsPattern = /args:\s*(\{[\s\S]*)/;
  const argsMatch = toolCallContent?.match(argsPattern); // Optional chaining

  if (!nameMatch) {
    log(`Invalid tool call format - missing name: ${toolCallText}`);
    return null;
  }

  const name = nameMatch[1]?.trim() || "";

  // If we're missing args, try to extract just the name
  if (!argsMatch) {
    log(`Invalid tool call format - missing args: ${toolCallText}`);
    // For tools without args, we could still proceed with empty args
    return { name, args: {}, toolCallText };
  }

  // Process args with more robust parsing
  let argsStr = argsMatch[1] || "{}";

  try {
    // Fix common JSON issues in the args string
    argsStr = fixJsonString(argsStr);

    // Parse the args as JSON
    const args = JSON.parse(argsStr);
    return { name, args, toolCallText };
  } catch (err) {
    log(`Error parsing tool call args: ${err}. Args string: ${argsStr}`);

    // Attempt a more forgiving parse for common issues
    try {
      // Try to parse it as a simplified object
      const fallbackArgs = parseSimplifiedArgs(argsStr);
      log(`Fallback parsing succeeded: ${JSON.stringify(fallbackArgs)}`);
      return { name, args: fallbackArgs, toolCallText };
    } catch (fallbackErr) {
      log(`Fallback parsing also failed: ${fallbackErr}`);
      // Return with empty args rather than failing completely
      return { name, args: {}, toolCallText };
    }
  }
}

/**
 * Fix common JSON string issues to make it more parsable
 */
function fixJsonString(jsonStr: string): string {
  // Extract just the JSON object part (handle case where there's text after the JSON)
  const match = jsonStr.match(/(\{[\s\S]*\})/);
  if (match && match[1]) {
    jsonStr = match[1];
  }

  // Fix missing quotes around property names
  jsonStr = jsonStr.replace(/(\s*)(\w+)(\s*):/g, '$1"$2"$3:');

  // Fix trailing commas in objects
  jsonStr = jsonStr.replace(/,(\s*[\}\]])/g, "$1");

  // Fix missing quotes around string values
  // This regex looks for property: value pairs where value isn't properly quoted
  jsonStr = jsonStr.replace(
    /"([^"]+)":\s*([^"{}\[\],\d][^{}\[\],\s]*)/g,
    '"$1": "$2"',
  );

  return jsonStr;
}

/**
 * Attempt to parse args in a more forgiving way for common model errors
 */
function parseSimplifiedArgs(argsStr: string): Record<string, any> {
  const result: Record<string, any> = {};

  // Look for key-value patterns like "key": "value" or "key": value
  const keyValuePattern =
    /"([^"]+)"\s*:\s*(?:"([^"]*)"|(true|false|null|\d+(?:\.\d+)?|\[[^\]]*\]|\{[^}]*\}))/g;

  let match;
  while ((match = keyValuePattern.exec(argsStr)) !== null) {
    const key = match[1];
    // If group 2 exists, it's a string value, otherwise use group 3 (non-string value)
    // Removed duplicate declaration: const key = match[1];
    let value: any = match[2] !== undefined ? match[2] : match[3]; // Use 'any' for value, ensure key is defined

    // For non-string values, try to parse them
    if (key === undefined) continue; // Skip if key is undefined

    if (match[2] === undefined && value !== undefined) { // Check value is defined
      try {
        // Parse numbers, booleans, etc.
        if (value === "true") {
          value = true as any; // Cast to any to satisfy potential downstream type checks if needed
        } else if (value === "false") {
          value = false as any;
        } else if (value === "null") {
          value = null as any;
        } else if (typeof value === 'string' && /^\d+$/.test(value)) { // Check type before test
          value = parseInt(value, 10); // Already checked value is string
        } else if (typeof value === 'string' && /^\d+\.\d+$/.test(value)) { // Check type before test
          value = parseFloat(value); // Already checked value is string
        } else if (typeof value === 'string' && (value.startsWith("[") || value.startsWith("{"))) { // Check type before startsWith
          value = JSON.parse(value); // Already checked value is string
        }
      } catch (e) {
        // Keep as string if parsing fails
      }
    }

    // Ensure key is valid before assignment (already checked above)
    result[key] = value;
  }

  // Special case for url and prompt which are common in fetch tools
  // Use bracket notation for index signatures
  if (argsStr.includes("url") && !result['url']) {
    const urlMatch = argsStr.match(/url['":\s]+([^"'\s,}]+)/i);
    if (urlMatch && urlMatch[1]) {
      result['url'] = urlMatch[1];
    }
  }

  if (argsStr.includes("prompt") && !result['prompt']) {
    const promptMatch = argsStr.match(/prompt['":\s]+"([^"]+)"/i);
    if (promptMatch && promptMatch[1]) {
      result['prompt'] = promptMatch[1];
    }
  }

  return result;
}

/**
 * Execute a tool call using the MCP registry
 */
export async function executeToolCall(
  toolCall: ToolCall,
  mcpManager: McpManager, // Use McpManager class type
): Promise<ToolCallResult> {
  const { name, args } = toolCall;

  log(`Executing tool call: ${name} with args: ${JSON.stringify(args)}`);

  try {
    // Check if this is an MCP tool
    if (name.startsWith("mcp__")) {
      // Execute the tool through the MCP registry
      // Parse mcpServerName and actualToolName from the namespaced name
      const nameParts = name.split("__");
      if (nameParts.length < 3 || nameParts[0] !== "mcp") {
        throw new Error(`Invalid MCP tool name format: ${name}`);
      }
      const mcpServerName = nameParts[1];
      const actualToolName = nameParts.slice(2).join("__"); // Handle tool names with underscores

      // Add explicit checks for mcpServerName and actualToolName
      if (!serverName || !actualToolName) {
        throw new Error(`Could not parsemcpServer and tool name from: ${name}`);
      }

      // Use the callTool method from McpManager
      const result = await mcpManager.callTool(mcpServerName, actualToolName, args);
      return {
        toolCall,
        ...result,
      };
    } else {
      // Not an MCP tool
      return {
        toolCall,
        error: `Unknown tool: ${name}. Tools must be prefixed with 'mcp__'.`,
      };
    }
  } catch (err) {
    log(`Error executing tool call: ${err}`);
    return {
      toolCall,
      error: `Error executing tool: ${err}`,
    };
  }
}

/**
 * Process a model response to identify and execute any tool calls
 */
export async function processToolCalls(
  response: string,
  mcpManager: McpManager, // Use McpManager class type
): Promise<{
  modifiedResponse: string;
  toolResults: Array<ToolCallResult>;
}> {
  const toolResults: Array<ToolCallResult> = [];
  let modifiedResponse = response;

  // Enhanced logging for debugging
  log(
    `Processing potential tool calls in: ${response.substring(0, 150)}${
      response.length > 150 ? "..." : ""
    }`,
  );

  // Find all tool calls in the response, handling case where model incorrectly adds text
  // Clean up the response to handle more model confusion patterns
  modifiedResponse = modifiedResponse
    // Remove any "codex" prefixes the model might add
    .replace(/^codex\s*/i, "")
    // Remove common explanatory phrases the model might add before tool calls
    .replace(
      /(?:I will|I'll|Let me|I'm going to|I can) (?:use|try|execute|call|utilize)(?: a| the)? (?:tool|function|method|api|fetch).*?(?=<tool>|$)/i,
      "",
    )
    // Remove any "tool call follows" messages
    .replace(/.*?tool call.*?:/i, "")
    // Remove any explanatory text before the actual tool call (more aggressive)
    .replace(/.*?<tool>/is, "<tool>");

  // Detect and fix malformed tool calls - common pattern is when model adds backticks
  if (modifiedResponse.includes("```") && modifiedResponse.includes("<tool>")) {
    modifiedResponse = modifiedResponse.replace(
      /```(?:xml|json)?([^`]*?<tool>[\s\S]*?<\/tool>)```/g,
      "$1",
    );
  }

  // Find all tool calls in the cleaned response
  const toolCallRegex = /<tool>[\s\S]*?<\/tool>/g;
  const toolCalls = modifiedResponse.match(toolCallRegex);

  if (!toolCalls || toolCalls.length === 0) {
    // If the model is clearly trying to use a tool but messed up syntax
    // Enhanced detection for broken tool syntax patterns
    if (
      modifiedResponse.includes("mcp__fetch__fetch") ||
      (modifiedResponse.includes("mcp__") &&
        (modifiedResponse.includes("args") ||
          modifiedResponse.includes("url") ||
          modifiedResponse.includes("{"))) ||
      // Check for partial tool syntax
      (modifiedResponse.includes("<tool>") &&
        !modifiedResponse.includes("</tool>")) ||
      (modifiedResponse.includes("name:") && modifiedResponse.includes("args:"))
    ) {
      log(
        `Model attempted to use tool but had incorrect syntax: ${modifiedResponse}`,
      );

      // Attempt to reconstruct a valid tool call from broken syntax
      try {
        // Extract potential tool name
        const nameMatch = modifiedResponse.match(/name:\s*([^\n,}\s]+)/);
        if (nameMatch && nameMatch[1] && nameMatch[1].includes("mcp__")) {
          const name = nameMatch[1].trim();
          log(`Detected tool name from broken syntax: ${name}`);

          // Extract potential args
          let args = {};
          const argsMatch = modifiedResponse.match(/args:\s*(\{[\s\S]*?\})/);
          if (argsMatch && argsMatch[1]) {
            try {
              // Try to fix and parse the args
              const fixedArgsStr = fixJsonString(argsMatch[1]);
              args = JSON.parse(fixedArgsStr);
              log(`Extracted args from broken syntax: ${JSON.stringify(args)}`);
            } catch (e) {
              // If JSON parsing fails, try the simplified parser
              args = parseSimplifiedArgs(argsMatch[1]);
              log(
                `Extracted simplified args from broken syntax: ${JSON.stringify(
                  args,
                )}`,
              );
            }
          } else {
            // No JSON args found, try to extract URL directly
            const urlMatch = modifiedResponse.match(/url['":\s]+([^"'\s,}]+)/i);
            if (urlMatch && urlMatch[1]) {
              args = { url: urlMatch[1] };
              log(`Extracted URL directly from broken syntax: ${urlMatch[1]}`);
            }

            // Try to extract prompt directly
            const promptMatch = modifiedResponse.match(
              /prompt['":\s]+"([^"]+)"/i,
            );
            if (promptMatch && promptMatch[1]) {
              args = { ...args, prompt: promptMatch[1] };
              log(
                `Extracted prompt directly from broken syntax: ${promptMatch[1]}`,
              );
            }
          }

          // Reconstruct a valid tool call and insert it into the response
          if (Object.keys(args).length > 0) {
            const reconstructedToolCall = `<tool>\nname: ${name}\nargs: ${JSON.stringify(
              args,
              null,
              2,
            )}\n</tool>`;
            log(`Reconstructed tool call: ${reconstructedToolCall}`);

            // Execute the reconstructed tool call
            const toolCall = {
              name,
              args,
              toolCallText: reconstructedToolCall,
            };
            const result = await executeToolCall(toolCall, mcpManager); // Pass mcpManager
            toolResults.push(result);

            // Replace the entire response with the tool result
            let resultText: string;
            if (result.error) {
              resultText = `I couldn't retrieve that information: ${result.error}`;
            } else {
              resultText = JSON.stringify(result.result, null, 2);
            }

            modifiedResponse = resultText;
            return { modifiedResponse, toolResults };
          }
        }
      } catch (e) {
        log(`Error reconstructing tool call from broken syntax: ${e}`);
      }
    }
    return { modifiedResponse, toolResults };
  }

  // Process each tool call
  for (const toolCallText of toolCalls) {
    const toolCall = parseToolCall(toolCallText);

    if (!toolCall) {
      // Replace invalid tool call with error message
      modifiedResponse = modifiedResponse.replace(
        toolCallText,
        `I couldn't process that request properly.`,
      );
      continue;
    }

    // Execute the tool
    const result = await executeToolCall(toolCall, mcpManager); // Pass mcpManager
    toolResults.push(result);

    // Replace the tool call with the result in a natural way
    let resultText: string;
    if (result.error) {
      // Include error for debugging but don't make it too technical
      resultText = `I couldn't retrieve that information: ${result.error}`;
    } else {
      // Simply include the raw result without any wrapper or indicators
      // This lets the model seamlessly incorporate the results in natural language
      resultText = JSON.stringify(result.result, null, 2);
    }

    // Replace entire response with just the result to avoid model confusion
    if (modifiedResponse.trim() === toolCallText.trim()) {
      modifiedResponse = resultText;
    } else {
      modifiedResponse = modifiedResponse.replace(toolCallText, resultText);
    }
  }

  return { modifiedResponse, toolResults };
}

/**
 * Format a tool result for display in the chat UI
 */
export function formatToolResult(result: ToolCallResult): string {
  const { toolCall, result: toolResult, error } = result;

  if (error) {
    return `🔧 Tool Call Error: ${toolCall.name}\nArgs: ${JSON.stringify(
      toolCall.args,
      null,
      2,
    )}\nError: ${error}`;
  }

  return `🔧 Tool Call Result: ${toolCall.name}\nArgs: ${JSON.stringify(
    toolCall.args,
    null,
    2,
  )}\nResult: ${JSON.stringify(toolResult, null, 2)}`;
}
