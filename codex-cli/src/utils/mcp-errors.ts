/**
 * Base error class for all MCP-related errors
 */
export class McpError extends Error {
  public override cause?: Error;
  constructor(message: string) {
    super(message);
    this.name = "McpError";

    // Ensure proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, McpError.prototype);
  }
}

/**
 * Error thrown when there's an issue connecting to an MCP server
 */
export class McpConnectionError extends McpError {
  public mcpServerName: string;
  public override cause?: Error;

  constructor(mcpServerName: string, cause?: Error) {
    const message = cause
      ? `Failed to connect to MCP Server '${mcpServerName}': ${cause.message}`
      : `Failed to connect to MCP Server '${mcpServerName}'`;

    super(message);
    this.name = "McpConnectionError";
    this.mcpServerName = mcpServerName;
    this.cause = cause;

    // Ensure proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, McpConnectionError.prototype);
  }
}

/**
 * Error thrown when there's an issue executing an MCP tool
 */
export class McpToolError extends McpError {
  public mcpServerName: string;
  public toolName: string;
  public override cause?: Error;

  constructor(mcpServerName: string, toolName: string, cause?: Error) {
    const message = cause
      ? `Error executing tool '${toolName}' onmcpServer '${mcpServerName}': ${cause.message}`
      : `Error executing tool '${toolName}' onmcpServer '${mcpServerName}'`;

    super(message);
    this.name = "McpToolError";
    this.mcpServerName = mcpServerName;
    this.toolName = toolName;
    this.cause = cause;

    // Ensure proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, McpToolError.prototype);
  }
}

/**
 * Error thrown when an MCP Server or tool is not found
 */
export class McpNotFoundError extends McpError {
  constructor(
    entityType: "server" | "tool",
    entityName: string,
    extraContext?: string,
  ) {
    const message = `Mcp ${entityType} '${entityName}' not found${
      extraContext ? `: ${extraContext}` : ""
    }`;

    super(message);
    this.name = "McpNotFoundError";

    // Ensure proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, McpNotFoundError.prototype);
  }
}

/**
 * Error thrown when an MCP operation times out
 */
export class McpTimeoutError extends McpError {
  public mcpServerName: string;
  public operationType: string;
  public timeoutMs: number;

  constructor(mcpServerName: string, operationType: string, timeoutMs: number) {
    super(
      `Mcp operation '${operationType}' onmcpServer '${mcpServerName}' timed out after ${timeoutMs}ms`,
    );
    this.name = "McpTimeoutError";
    this.mcpServerName = mcpServerName;
    this.operationType = operationType;
    this.timeoutMs = timeoutMs;

    // Ensure proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, McpTimeoutError.prototype);
  }
}

/**
 * Error thrown when an MCP Server returns an invalid response
 */
export class McpInvalidResponseError extends McpError {
  public mcpServerName: string;
  public responseData?: any;

  constructor(mcpServerName: string, message: string, responseData?: any) {
    super(`Invalid response from MCP Server '${mcpServerName}': ${message}`);
    this.name = "McpInvalidResponseError";
    this.mcpServerName = mcpServerName;
    this.responseData = responseData;

    // Ensure proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, McpInvalidResponseError.prototype);
  }
}
