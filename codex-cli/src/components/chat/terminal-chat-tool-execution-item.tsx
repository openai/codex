import type { ToolCallResult } from "../../utils/agent/tool-executor";

import { Box, Text } from "ink";
import React from "react";

interface ToolExecutionItemProps {
  toolResult: ToolCallResult;
  loading?: boolean;
}

/**
 * Component to display a tool execution in the chat
 */
export function TerminalChatToolExecutionItem({
  toolResult,
  loading = false,
}: ToolExecutionItemProps): JSX.Element {
  const { toolCall, result, error } = toolResult;
  const isError = Boolean(error);

  // If this is a debug build or explicitly showing tool execution details
  const showDebugInfo =
    process.env["NODE_ENV"] === "development" ||
    process.env["SHOW_TOOL_DETAILS"] === "true";

  if (showDebugInfo) {
    return (
      <Box
        flexDirection="column"
        borderStyle="round"
        borderColor={isError ? "red" : "blue"}
        padding={1}
      >
        <Box>
          <Text bold color={isError ? "red" : "blue"}>
            🔧 Tool Call:{" "}
            <Text bold color="white">
              {toolCall.name}
            </Text>
            {loading && <Text color="yellow"> (running...)</Text>}
          </Text>
        </Box>

        <Box flexDirection="column" marginLeft={2}>
          <Text>Arguments:</Text>
          <Box marginLeft={2}>
            <Text>{JSON.stringify(toolCall.args, null, 2)}</Text>
          </Box>

          {/* Show result or error */}
          {(result !== undefined || error !== undefined) && (
            <>
              <Text>{isError ? "Error:" : "Result:"}</Text>
              <Box marginLeft={2}>
                <Text color={isError ? "red" : undefined}>
                  {isError
                    ? error
                    : typeof result === "object"
                    ? JSON.stringify(result, null, 2)
                    : String(result)}
                </Text>
              </Box>
            </>
          )}
        </Box>
      </Box>
    );
  } else {
    // In production, show a more user-friendly, simplified indicator
    return (
      <Box flexDirection="column" padding={1}>
        <Text color={isError ? "red" : "blue"}>
          {isError
            ? "⚠️ I encountered an issue retrieving that information."
            : "📊 Retrieved external information..."}
          {loading && <Text color="yellow"> (retrieving...)</Text>}
        </Text>
      </Box>
    );
  }
}

/**
 * Props for the async tool execution component
 */
interface AsyncToolExecutionProps {
  toolName: string;
  args: Record<string, any>;
  status: "running" | "success" | "error";
  result?: any;
  error?: string;
}

/**
 * Component to display an async tool execution
 */
export function AsyncToolExecution({
  toolName,
  args,
  status,
  result,
  error,
}: AsyncToolExecutionProps): JSX.Element {
  // If this is a debug build or explicitly showing tool execution details
  const showDebugInfo =
    process.env["NODE_ENV"] === "development" ||
    process.env["SHOW_TOOL_DETAILS"] === "true";

  if (showDebugInfo) {
    return (
      <Box
        flexDirection="column"
        borderStyle="round"
        borderColor={status === "error" ? "red" : "blue"}
        padding={1}
      >
        <Box>
          <Text bold color={status === "error" ? "red" : "blue"}>
            🔧 Tool Call:{" "}
            <Text bold color="white">
              {toolName}
            </Text>
            {status === "running" && <Text color="yellow"> (running...)</Text>}
          </Text>
        </Box>

        <Box flexDirection="column" marginLeft={2}>
          <Text>Arguments:</Text>
          <Box marginLeft={2}>
            <Text>{JSON.stringify(args, null, 2)}</Text>
          </Box>

          {/* Show result or error based on status */}
          {status !== "running" && (
            <>
              <Text>{status === "error" ? "Error:" : "Result:"}</Text>
              <Box marginLeft={2}>
                <Text color={status === "error" ? "red" : undefined}>
                  {status === "error"
                    ? error
                    : typeof result === "object"
                    ? JSON.stringify(result, null, 2)
                    : String(result)}
                </Text>
              </Box>
            </>
          )}
        </Box>
      </Box>
    );
  } else {
    // In production, show a more user-friendly, simplified indicator
    return (
      <Box flexDirection="column" padding={1}>
        <Text color={status === "error" ? "red" : "blue"}>
          {status === "error"
            ? "⚠️ I encountered an issue retrieving that information."
            : status === "running"
            ? "🔍 Retrieving information..."
            : "📊 Retrieved external information"}
        </Text>
      </Box>
    );
  }
}
