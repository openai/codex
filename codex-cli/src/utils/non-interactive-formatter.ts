import type { ResponseItem } from "openai/resources/responses/responses";
import { parseToolCall } from "./parsers";

/**
 * Format response items for non-interactive mode with cleaner output
 * Filters out reasoning messages and improves readability
 */
export function formatForNonInteractiveMode(item: ResponseItem): string | null {
  // Skip reasoning messages entirely
  // @ts-expect-error - The type might not be in ResponseItem
  if (item.type === "reasoning") {
    return null;
  }

  switch (item.type) {
    case "message": {
      const role = item.role === "assistant" ? "assistant" : item.role;
      const txt = item.content
        .map((c) => {
          if (c.type === "output_text" || c.type === "input_text") {
            return c.text;
          }
          if (c.type === "input_image") {
            return "<Image>";
          }
          if (c.type === "input_file") {
            return c.filename;
          }
          if (c.type === "refusal") {
            return c.refusal;
          }
          return "?";
        })
        .join(" ");
      return `${role}: ${txt}`;
    }
    case "function_call": {
      const details = parseToolCall(item);
      return `> Running: ${details?.cmdReadableText ?? item.name}`;
    }
    case "function_call_output": {
      // @ts-expect-error metadata unknown
      const meta = item.metadata as { exit_code?: number; duration_seconds?: number };
      
      try {
        // Parse the JSON output to get the execution status
        const outputData = JSON.parse(item.output);
        const exitCode = meta?.exit_code ?? outputData.metadata?.exit_code;
        
        // Just show execution status without the actual output
        if (exitCode === 0 || exitCode === undefined) {
          return "Command executed successfully";
        } else {
          return `Command failed with exit code: ${exitCode}`;
        }
      } catch (e) {
        // Fall back to a simple message if parsing fails
        return "Command execution completed";
      }
    }
    default: {
      return JSON.stringify(item);
    }
  }
} 