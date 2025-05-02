import type { ApplyPatchCommand, ApprovalPolicy } from "../approvals.js";
import type { CommandConfirmation } from "../utils/agent/agent-loop.js";
import type { AppConfig } from "../utils/config.js";
import type {
  FunctionTool,
  ResponseInputItem,
} from "openai/resources/responses/responses.mjs";

/**
 * Shared tool registry for Codex CLI.
 *
 * Stores function-callable tools along with their execution logic.
 * Each tool is defined by an OpenAI-compatible schema and a handler function.
 */

/**
 * Context passed to each registered tool handler.
 */
export type ToolHandlerContext = {
  config: AppConfig;
  approvalPolicy: ApprovalPolicy;
  cwd: string;
  signal?: AbortSignal;
  additionalWritableRoots: ReadonlyArray<string>;
  getCommandConfirmation: (
    command: Array<string>,
    patch?: ApplyPatchCommand,
  ) => Promise<CommandConfirmation>;
};

/**
 * Result returned by a tool handler.
 *
 * - `output`: The main result string to include in function_call_output.
 * - `additionalItems`: Optional list of extra items to be streamed alongside (e.g., patches, messages).
 */
export type ToolHandlerResult = {
  output: string;
  additionalItems?: Array<ResponseInputItem>;
};

/**
 * ToolHandler represents a function-callable unit of work.
 */
export type ToolHandler = (
  args: Record<string, unknown>,
  context: ToolHandlerContext,
) => Promise<ToolHandlerResult>;

/**
 * A complete tool, combining schema definition and execution logic.
 */
export interface RegisteredTool {
  definition: FunctionTool;
  handler: ToolHandler;
  aliases?: Array<string>;
}

// In-memory tool registry
const toolRegistry: Record<string, RegisteredTool> = {};

/**
 * Maps alias tool names to their canonical tool names.
 *
 * Aliases are used to support legacy or semantic tool identifiers (e.g. "container.exec")
 * This map is only used for internal lookup and is not passed to the model.
 */
const toolAliasMap: Record<string, string> = {};

/**
 * Registers a tool definition and its associated handler.
 */
export function registerTool(tool: RegisteredTool): void {
  const { definition, handler, aliases = [] } = tool;
  const name = definition.name;

  if (!/^[a-zA-Z0-9_-]+$/.test(name)) {
    throw new Error(
      `Tool name "${name}" is invalid. Must match /^[a-zA-Z0-9_-]+$/`,
    );
  }

  toolRegistry[name] = { definition, handler, aliases };

  for (const alias of aliases) {
    toolAliasMap[alias] = name;
  }
}

/**
 * Retrieves the registered handler for a tool.
 */
export function getToolHandler(name: string): ToolHandler | undefined {
  const resolvedName = toolAliasMap[name] || name;
  return toolRegistry[resolvedName]?.handler;
}

/**
 * Returns all registered tool schemas (to be passed to OpenAI).
 */
export function getRegisteredToolDefinitions(): Array<FunctionTool> {
  return Object.values(toolRegistry).map((t) => t.definition);
}

/**
 * Returns the names of all registered tools.
 */
export function getRegisteredToolNames(): Array<string> {
  return Object.keys(toolRegistry);
}

/**
 * Returns tool definitions along with their registered aliases.
 */
export function getToolSummaries(): Array<{
  name: string;
  description?: string;
  aliases?: Array<string>;
}> {
  return Object.entries(toolRegistry).map(
    ([name, { definition, aliases }]) => ({
      name,
      description: definition.description ?? undefined,
      aliases,
    }),
  );
}
