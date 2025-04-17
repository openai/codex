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
  public serverName: string;
  public override cause?: Error;

  constructor(serverName: string, cause?: Error) {
    const message = cause
      ? `Failed to connect to Mcp server '${serverName}': ${cause.message}`
      : `Failed to connect to Mcp server '${serverName}'`;

    super(message);
    this.name = "McpConnectionError";
    this.serverName = serverName;
    this.cause = cause;

    // Ensure proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, McpConnectionError.prototype);
  }
}

/**
 * Error thrown when there's an issue executing an MCP tool
 */
export class McpToolError extends McpError {
  public serverName: string;
  public toolName: string;
  public override cause?: Error;

  constructor(serverName: string, toolName: string, cause?: Error) {
    const message = cause
      ? `Error executing tool '${toolName}' on server '${serverName}': ${cause.message}`
      : `Error executing tool '${toolName}' on server '${serverName}'`;

    super(message);
    this.name = "McpToolError";
    this.serverName = serverName;
    this.toolName = toolName;
    this.cause = cause;

    // Ensure proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, McpToolError.prototype);
  }
}

/**
 * Error thrown when an MCP server or tool is not found
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
  public serverName: string;
  public operationType: string;
  public timeoutMs: number;

  constructor(serverName: string, operationType: string, timeoutMs: number) {
    super(
      `Mcp operation '${operationType}' on server '${serverName}' timed out after ${timeoutMs}ms`,
    );
    this.name = "McpTimeoutError";
    this.serverName = serverName;
    this.operationType = operationType;
    this.timeoutMs = timeoutMs;

    // Ensure proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, McpTimeoutError.prototype);
  }
}

/**
 * Error thrown when an MCP server returns an invalid response
 */
export class McpInvalidResponseError extends McpError {
  public serverName: string;
  public responseData?: any;

  constructor(serverName: string, message: string, responseData?: any) {
    super(`Invalid response from Mcp server '${serverName}': ${message}`);
    this.name = "McpInvalidResponseError";
    this.serverName = serverName;
    this.responseData = responseData;

    // Ensure proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, McpInvalidResponseError.prototype);
  }
}
