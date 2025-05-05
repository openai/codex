// Special delimiter that's unlikely to appear in normal tool names
// Using "__MCP__" as a distinctive separator that's valid in OpenAI's pattern
export const SERVER_TOOL_DELIMITER_SPACE = "__MCP__SPACE__";
export const SERVER_TOOL_DELIMITER_UNDERSCORE = "__MCP__UNDERSCORE__";
export const SERVER_TOOL_DELIMITER_HYPHEN = "__MCP__HYPHEN__";
export const SERVER_TOOL_DELIMITER_DOT = "__MCP__DOT__";
export const SERVER_TOOL_DELIMITER = "__MCP__TOOL__";

const replaceMap = {
  " ": SERVER_TOOL_DELIMITER_SPACE,
  "-": SERVER_TOOL_DELIMITER_HYPHEN,
  "_": SERVER_TOOL_DELIMITER_UNDERSCORE,
  ".": SERVER_TOOL_DELIMITER_DOT,
};

// Create reverse mapping for unsanitizing
const reverseReplaceMap: Record<string, string> = {};
Object.entries(replaceMap).forEach(([key, value]) => {
  reverseReplaceMap[value] = key;
});

/**
 * Creates a regex pattern that matches all delimiter tokens
 * @returns RegExp that matches all delimiter tokens
 */
function createDelimiterPattern(): RegExp {
  return new RegExp(
    Object.values(replaceMap)
      .map((token) => token.replace(/[-[\]{}()*+?.,\\^$|#\s]/g, "\\$&"))
      .join("|"),
    "g",
  );
}

/**
 * Replaces special characters in a name with their corresponding delimiter tokens
 * @param name The original name to sanitize
 * @returns Sanitized name with special characters replaced by delimiter tokens
 */
function replaceSpecialChars(name: string): string {
  return name.replace(/[^a-zA-Z0-9_-]/g, (match) => {
    const replacement = replaceMap[match as keyof typeof replaceMap];
    if (!replacement) {
      throw new Error(
        `Unexpected character: ${match}. Make sure to use a valid character in your tool name.`,
      );
    }
    return replacement;
  });
}

/**
 * Replaces delimiter tokens in a name with their original characters
 * @param name The sanitized name with delimiter tokens
 * @returns Original name with delimiter tokens replaced by their original characters
 */
function restoreOriginalChars(name: string): string {
  const delimiterPattern = createDelimiterPattern();
  return name.replace(
    delimiterPattern,
    (match) => reverseReplaceMap[match] || match,
  );
}

/**
 * Splits a sanitized tool name into server name and tool name components
 * @param sanitizedName The sanitized tool name with SERVER_TOOL_DELIMITER
 * @returns Object with serverName and toolName properties
 */
function splitSanitizedName(sanitizedName: string): {
  serverName: string;
  toolName: string;
} {
  let serverName: string;
  let toolName: string;

  if (sanitizedName.includes(SERVER_TOOL_DELIMITER)) {
    const parts = sanitizedName.split(SERVER_TOOL_DELIMITER);
    serverName = parts[0] || "";
    toolName = parts[1] || "";
  } else {
    toolName = sanitizedName;
    serverName = "";
  }

  if (!toolName) {
    throw new Error("Invalid sanitized tool name. Must contain a tool name.");
  }

  return { serverName, toolName };
}

/**
 * Ensures the tool name complies with OpenAI's pattern requirement: ^[a-zA-Z0-9_-]+$
 * This replaces any non-compliant characters with specialized delimiter tokens
 * @param name The original tool name
 * @returns Sanitized tool name compliant with OpenAI's pattern
 */
export function sanitizeToolName(name: string): string {
  return replaceSpecialChars(name);
}

/**
 * Reconstructs the original name by replacing the delimiters with the original characters
 * @param sanitizedName The sanitized tool name with delimiter tokens
 * @returns Object with serverName and toolName properties with original characters restored
 */
export function unsanitizeToolName(sanitizedName: string): {
  serverName: string;
  toolName: string;
} {
  const { serverName, toolName } = splitSanitizedName(sanitizedName);

  return {
    serverName: restoreOriginalChars(serverName),
    toolName: restoreOriginalChars(toolName),
  };
}
