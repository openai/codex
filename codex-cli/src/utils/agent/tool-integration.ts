/**
 * Tool integration module for Codex CLI
 *
 * This module handles the integration of additional tools into the agent loop
 * for use with different model providers.
 */

import type { ApprovalPolicy } from "../../approvals.js";
import type { AppConfig } from "../config.js";
import type { FunctionTool } from "openai/resources/responses/responses.mjs";

/**
 * Get the complete set of tools available to the agent based on configuration
 */
export function getAvailableTools(
  config: AppConfig,
  supportsBrowserUse = true,
): Array<FunctionTool> {
  // Base shell tool is always available
  const tools: Array<FunctionTool> = [
    {
      type: "function",
      name: "shell",
      description: "Runs a shell command, and returns its output.",
      strict: false,
      parameters: {
        type: "object",
        properties: {
          command: { type: "array", items: { type: "string" } },
          workdir: {
            type: "string",
            description: "The working directory for the command.",
          },
          timeout: {
            type: "number",
            description:
              "The maximum time to wait for the command to complete in milliseconds.",
          },
        },
        required: ["command"],
        additionalProperties: false,
      },
    },
  ];

  // Add list_code_definition_names tool
  tools.push({
    type: "function",
    name: "list_code_definition_names",
    description:
      "Lists all definitions (classes, functions, methods, etc.) used in source code files at the top level of the specified directory.",
    strict: false,
    parameters: {
      type: "object",
      properties: {
        path: {
          type: "string",
          description:
            "The path of the directory to list top level source code definitions for.",
        },
      },
      required: ["path"],
      additionalProperties: false,
    },
  });

  // Add ask_followup_question tool
  tools.push({
    type: "function",
    name: "ask_followup_question",
    description:
      "Asks the user a follow-up question to gather additional information needed to complete the task.",
    strict: false,
    parameters: {
      type: "object",
      properties: {
        question: {
          type: "string",
          description: "The question to ask the user.",
        },
        options: {
          type: "array",
          items: { type: "string" },
          description:
            "Optional array of 2-5 options for the user to choose from.",
        },
      },
      required: ["question"],
      additionalProperties: false,
    },
  });

  // Add attempt_completion tool
  tools.push({
    type: "function",
    name: "attempt_completion",
    description: "Present the final result of the task to the user.",
    strict: false,
    parameters: {
      type: "object",
      properties: {
        result: {
          type: "string",
          description: "The result of the task.",
        },
        command: {
          type: "string",
          description:
            "Optional CLI command to show a live demo of the result.",
        },
      },
      required: ["result"],
      additionalProperties: false,
    },
  });

  // Add browser_action tool if browser support is enabled
  if (supportsBrowserUse) {
    tools.push({
      type: "function",
      name: "browser_action",
      description: "Interact with a Puppeteer-controlled browser.",
      strict: false,
      parameters: {
        type: "object",
        properties: {
          action: {
            type: "string",
            enum: [
              "launch",
              "click",
              "type",
              "scroll_down",
              "scroll_up",
              "close",
            ],
            description: "The action to perform with the browser.",
          },
          url: {
            type: "string",
            description:
              "The URL to launch the browser at (for 'launch' action only).",
          },
          coordinate: {
            type: "string",
            description:
              "The x,y coordinates for clicking (for 'click' action only).",
          },
          text: {
            type: "string",
            description: "The text to type (for 'type' action only).",
          },
        },
        required: ["action"],
        additionalProperties: false,
      },
    });
  }

  // Add MCP tools if config indicates MCP should be enabled
  // We check for the existence of MCP-related properties in the config
  const extendedConfig = config as {
    mcpServers?: unknown;
    mcpEnabled?: boolean;
  };
  if (extendedConfig.mcpServers || extendedConfig.mcpEnabled) {
    // use_mcp_tool
    tools.push({
      type: "function",
      name: "use_mcp_tool",
      description: "Use a tool provided by a connected MCP server.",
      strict: false,
      parameters: {
        type: "object",
        properties: {
          server_name: {
            type: "string",
            description: "The name of the MCP server providing the tool.",
          },
          tool_name: {
            type: "string",
            description: "The name of the tool to execute.",
          },
          arguments: {
            type: "string",
            description:
              "A JSON string containing the tool's input parameters.",
          },
        },
        required: ["server_name", "tool_name", "arguments"],
        additionalProperties: false,
      },
    });

    // access_mcp_resource
    tools.push({
      type: "function",
      name: "access_mcp_resource",
      description: "Access a resource provided by a connected MCP server.",
      strict: false,
      parameters: {
        type: "object",
        properties: {
          server_name: {
            type: "string",
            description: "The name of the MCP server providing the resource.",
          },
          uri: {
            type: "string",
            description: "The URI identifying the specific resource to access.",
          },
        },
        required: ["server_name", "uri"],
        additionalProperties: false,
      },
    });
  }

  return tools;
}

// All tool arguments are just Record<string, unknown>
// We use type guards to check for specific properties

/**
 * Response items for tools
 */
interface ToolResponseItem {
  type: string;
  role: string;
  content: Array<{
    type: string;
    text: string;
  }>;
}

interface ToolResponseMetadata {
  success: boolean;
  [key: string]: unknown;
}

// Helper functions to check if required properties are present
function hasPath(args: Record<string, unknown>): boolean {
  return typeof args["path"] === "string";
}

function hasQuestion(args: Record<string, unknown>): boolean {
  return typeof args["question"] === "string";
}

function hasResult(args: Record<string, unknown>): boolean {
  return typeof args["result"] === "string";
}

function hasAction(args: Record<string, unknown>): boolean {
  return typeof args["action"] === "string";
}

function hasMcpToolProperties(args: Record<string, unknown>): boolean {
  return (
    typeof args["server_name"] === "string" &&
    typeof args["tool_name"] === "string" &&
    typeof args["arguments"] === "string"
  );
}

function hasMcpResourceProperties(args: Record<string, unknown>): boolean {
  return (
    typeof args["server_name"] === "string" && typeof args["uri"] === "string"
  );
}

/**
 * Process tool calls with appropriate handlers
 *
 * This function handles different tool types with proper type checking
 */
export async function handleToolCall(
  name: string,
  args: Record<string, unknown>,
  _config: AppConfig,
  _approvalPolicy: ApprovalPolicy,
  _signal?: AbortSignal,
): Promise<{
  output: string;
  metadata?: ToolResponseMetadata;
  additionalItems?: Array<ToolResponseItem>;
}> {
  // Default result structure
  const result = {
    output: "Tool not implemented",
    metadata: { success: false } as ToolResponseMetadata,
    additionalItems: [] as Array<ToolResponseItem>,
  };

  // Handle specialized tools
  switch (name) {
    case "list_code_definition_names":
      if (hasPath(args)) {
        try {
          // In a real implementation, this would call into a code analysis module
          result.output = `Code definitions would be listed for path: ${args["path"]}`;
          result.metadata = { success: true, path: args["path"] };
        } catch (error) {
          const errorMsg =
            error instanceof Error ? error.message : String(error);
          result.output = `Error listing code definitions: ${errorMsg}`;
          result.metadata = { success: false, error: errorMsg };
        }
      } else {
        result.output = "Missing required parameter: path";
      }
      break;

    case "ask_followup_question":
      if (hasQuestion(args)) {
        // This would be handled by the UI in a real implementation
        result.output = `Followup question asked: ${args["question"]}`;
        result.metadata = {
          success: true,
          question: args["question"],
          options: args["options"] || [],
        };

        // Add an item to the response stream
        result.additionalItems = [
          {
            type: "message",
            role: "assistant",
            content: [
              {
                type: "input_text",
                text: `I need to ask a follow-up question: ${args["question"]}`,
              },
            ],
          },
        ];
      } else {
        result.output = "Missing required parameter: question";
      }
      break;

    case "attempt_completion":
      if (hasResult(args)) {
        const resultString = args["result"] as string;
        result.output = `Task completion attempted with result: ${resultString.substring(0, 50)}...`;
        result.metadata = {
          success: true,
          result: args["result"],
          command: args["command"] || null,
        };

        // Add the completion message to the response
        result.additionalItems = [
          {
            type: "message",
            role: "assistant",
            content: [
              {
                type: "input_text",
                text: `Task completion: ${args["result"]}`,
              },
            ],
          },
        ];

        // If there's a command to demonstrate the result, add it
        if (args["command"]) {
          result.additionalItems.push({
            type: "message",
            role: "system",
            content: [
              {
                type: "input_text",
                text: `Running demonstration command: ${args["command"]}`,
              },
            ],
          });
        }
      } else {
        result.output = "Missing required parameter: result";
      }
      break;

    case "browser_action":
      if (hasAction(args)) {
        // Here we would interact with a Puppeteer browser
        result.output = `Browser action requested: ${args["action"]}`;
        result.metadata = {
          success: true,
          action: args["action"],
          url: args["url"] || null,
          coordinate: args["coordinate"] || null,
          text: args["text"] || null,
        };
      } else {
        result.output = "Missing required parameter: action";
      }
      break;

    case "use_mcp_tool":
      if (hasMcpToolProperties(args)) {
        // Here we would call the MCP server's tool
        result.output = `MCP tool call requested: ${args["server_name"]}/${args["tool_name"]}`;
        result.metadata = {
          success: true,
          server_name: args["server_name"],
          tool_name: args["tool_name"],
          arguments: args["arguments"],
        };
      } else {
        result.output = "Missing required parameters for MCP tool call";
      }
      break;

    case "access_mcp_resource":
      if (hasMcpResourceProperties(args)) {
        // Here we would access the MCP server's resource
        result.output = `MCP resource access requested: ${args["server_name"]} - ${args["uri"]}`;
        result.metadata = {
          success: true,
          server_name: args["server_name"],
          uri: args["uri"],
        };
      } else {
        result.output = "Missing required parameters for MCP resource access";
      }
      break;

    default:
      result.output = `Unknown tool: ${name}`;
      break;
  }

  return result;
}
