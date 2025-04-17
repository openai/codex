import type { MCPServer as McpServerConfig } from "./mcp";

import { listServers } from "./mcp";
import {
  McpError,
  McpConnectionError,
  McpToolError,
  McpNotFoundError,
} from "./mcp-errors";
import {
  debug,
  info,
  warn,
  error as logError,
  LogLevel,
  setLogLevel,
  enableConsoleDebug,
} from "./mcp-logger";
import { McpStdioClient } from "./mcp-stdio-client";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { SSEClientTransport } from "@modelcontextprotocol/sdk/client/sse.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import {
  CallToolResultSchema,
  ListToolsResultSchema,
} from "@modelcontextprotocol/sdk/types.js";

// Define internal connection state
type McpConnection = {
  serverConfig: McpServerConfig;
  client: Client;
  transport: StdioClientTransport | SSEClientTransport;
  stdioClient?: McpStdioClient;
  status: "connecting" | "connected" | "disconnected" | "error";
  error?: string;
  tools: Array<McpToolDefinition>;
  lastConnectionAttempt?: number; // Track reconnection attempts
  connectionAttempts: number; // Count connection attempts for backoff
};

export interface McpToolDefinition {
  name: string;
  description: string;
  parameters: any; // JSON Schema object
}

export interface McpToolResult {
  result?: any;
  error?: string;
  partial?: any; // For streaming results
}

// Configuration constants
const DEFAULT_REQUEST_TIMEOUT_MS = 5000;
const DEFAULT_TOOL_EXEC_TIMEOUT_MS = 30000;
const MAX_CONNECTION_RETRIES = 3;
const INITIAL_RETRY_DELAY_MS = 500;

/**
 * A utility function to execute an operation with proper error handling
 * @param operation The async operation to execute
 * @param errorHandler Function to handle errors
 * @returns The result of the operation or error handler
 */
async function executeWithErrorHandling<T>(
  operation: () => Promise<T>,
  errorHandler: (error: Error) => T,
): Promise<T> {
  try {
    return await operation();
  } catch (err) {
    const error = err instanceof Error ? err : new Error(String(err));
    logError(error.message);
    return errorHandler(error);
  }
}

export class McpManager {
  private connections: Map<string, McpConnection> = new Map();
  private isInitialized = false;
  private clientVersion: string = "1.0.0";
  private hardAbort?: AbortController;
  private debugEnabled = false;

  constructor(options?: { debugMode?: boolean }) {
    this.hardAbort = new AbortController();

    if (options?.debugMode) {
      this.enableDebugLogging(true);
    }

    // Load debug settings from environment
    if (process.env["MCP_DEBUG"] === "1") {
      this.enableDebugLogging(true);
    }
  }

  /**
   * Enable or disable detailed debug logging
   */
  public enableDebugLogging(enabled: boolean): void {
    this.debugEnabled = enabled;
    setLogLevel(enabled ? LogLevel.DEBUG : LogLevel.INFO);
    enableConsoleDebug(enabled);
    info(`Debug logging ${enabled ? "enabled" : "disabled"}`);
  }

  /**
   * Initialize connections to all configured MCP servers.
   */
  async initialize(): Promise<void> {
    if (this.isInitialized) {
      debug("Already initialized, skipping");
      return;
    }

    info("Initializing...");

    try {
      const localServers = await listServers("local");
      const globalServers = await listServers("global");
      const allServerConfigs = [...localServers, ...globalServers];

      // Deduplicate based on name, prioritizing local config
      const uniqueServerConfigs = new Map<string, McpServerConfig>();
      for (const server of allServerConfigs) {
        if (!uniqueServerConfigs.has(server.name)) {
          uniqueServerConfigs.set(server.name, server);
        }
      }

      if (uniqueServerConfigs.size === 0) {
        info("No Mcp servers configured");
        this.isInitialized = true;
        return;
      }

      info(`Found ${uniqueServerConfigs.size} servers. Connecting...`);

      // Connect to each server with retry logic
      const connectionPromises = Array.from(uniqueServerConfigs.values()).map(
        (config) =>
          this.connectWithRetry(config).catch((err) => {
            const errorMessage =
              err instanceof Error ? err.message : String(err);
            warn(`Failed to connect to server ${config.name}: ${errorMessage}`);

            // Store error state even if all connection attempts failed
            this.connections.set(config.name, {
              serverConfig: config,
              client: null as any,
              transport: null as any,
              status: "error",
              error: errorMessage,
              tools: [],
              connectionAttempts: 0,
              lastConnectionAttempt: Date.now(),
            });
          }),
      );

      await Promise.all(connectionPromises);

      this.isInitialized = true;
      info(
        `Initialization complete. ${this.connections.size} connections attempted.`,
      );
      this.logConnectionStatus();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      logError(`Error during initialization: ${errorMessage}`);

      // Still mark as initialized to prevent retries, but log the failure
      this.isInitialized = true;
    }
  }

  /**
   * Log a summary of connection statuses
   */
  private logConnectionStatus(): void {
    let connectedCount = 0;
    let errorCount = 0;

    this.connections.forEach((conn) => {
      if (conn.status === "connected") {
        connectedCount++;
      }
      if (conn.status === "error") {
        errorCount++;
      }
    });

    info(`Status: ${connectedCount} connected, ${errorCount} failed`);

    // Only log details at debug level
    if (this.debugEnabled) {
      this.connections.forEach((conn, name) => {
        debug(
          `Server '${name}': status=${conn.status}, tools=${conn.tools.length}${
            conn.error ? `, error=${conn.error}` : ""
          }`,
        );
      });
    }
  }

  /**
   * Connect to a server with retry logic
   */
  private async connectWithRetry(
    config: McpServerConfig,
    maxRetries: number = MAX_CONNECTION_RETRIES,
    initialBackoffMs: number = INITIAL_RETRY_DELAY_MS,
  ): Promise<void> {
    let lastError: Error | null = null;
    let conn = this.connections.get(config.name);

    // Track connection attempts
    if (!conn) {
      conn = {
        serverConfig: config,
        client: null as any,
        transport: null as any,
        status: "connecting",
        tools: [],
        connectionAttempts: 0,
        lastConnectionAttempt: Date.now(),
      };
      this.connections.set(config.name, conn);
    }

    // Increment the attempt counter - useful for progressive backoff
    conn.connectionAttempts += 1;
    conn.lastConnectionAttempt = Date.now();

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      try {
        debug(
          `Connection attempt ${attempt}/${maxRetries} to '${config.name}'`,
        );
        await this.connectToServer(config);
        info(`Connected to server '${config.name}' on attempt ${attempt}`);
        return;
      } catch (err) {
        lastError = err instanceof Error ? err : new Error(String(err));

        warn(
          `Connection attempt ${attempt}/${maxRetries} to '${config.name}' failed: ${lastError.message}`,
        );

        if (attempt < maxRetries) {
          // Exponential backoff with jitter
          const backoffMs = initialBackoffMs * Math.pow(2, attempt - 1);
          const jitter = Math.floor(Math.random() * 200); // Add up to 200ms of jitter
          const delayMs = backoffMs + jitter;

          debug(`Retrying in ${delayMs}ms...`);
          await new Promise((resolve) => setTimeout(resolve, delayMs));
        }
      }
    }

    // If we get here, all attempts failed
    throw new McpConnectionError(
      config.name,
      lastError || new Error("Max retries exceeded"),
    );
  }

  /**
   * Connect to a single MCP server based on its configuration.
   */
  private async connectToServer(config: McpServerConfig): Promise<void> {
    debug(`Connecting to ${config.name} (${config.type})...`);

    // Prevent duplicate connections
    const existingConn = this.connections.get(config.name);
    if (existingConn?.status === "connected") {
      debug(`Server ${config.name} already connected.`);
      return;
    }

    // Keep track of connections, even if they fail
    const conn: McpConnection = existingConn || {
      serverConfig: config,
      client: null as any,
      transport: null as any,
      status: "connecting",
      tools: [],
      connectionAttempts: 0,
      lastConnectionAttempt: Date.now(),
    };

    // Update existing connection or create a new one
    this.connections.set(config.name, conn);

    let client: Client;
    let transport: StdioClientTransport | SSEClientTransport;

    try {
      client = new Client(
        { name: "CodexCLI-Agent", version: this.clientVersion },
        { capabilities: {} },
      );

      if (config.type === "sse") {
        if (!config.url) {
          throw new McpConnectionError(
            config.name,
            new Error("SSE server is missing 'url' field"),
          );
        }
        transport = new SSEClientTransport(new URL(config.url), {});
      } else if (config.type === "stdio") {
        if (!config.cmd) {
          throw new McpConnectionError(
            config.name,
            new Error("Stdio server is missing 'cmd' field"),
          );
        }

        // Create the stdio client
        const stdioClient = new McpStdioClient(config.cmd, config.args || [], {
          serverName: config.name,
          env: config.env,
          debug: this.debugEnabled,
        });

        // Set up event listeners
        stdioClient.addEventListener((event) => {
          if (event.type === "log") {
            debug(`[${config.name}] ${event.message}`);
          } else if (event.type === "error") {
            warn(`[${config.name}] ${event.message}`);

            // Update error state if connection exists
            const existingConn = this.connections.get(config.name);
            if (existingConn) {
              existingConn.error = `${
                existingConn.error ? existingConn.error + "\n" : ""
              }${event.message}`;
            }
          } else if (event.type === "ready") {
            debug(`[${config.name}] Server ready event received`);
          }
        });

        // Start the client and wait for it to be ready
        await stdioClient.start();
        await stdioClient.waitForReady();

        // Create a dummy transport for the SDK
        transport = new StdioClientTransport({
          command: "echo",
          args: ["dummy"],
          stderr: "pipe",
        });

        // Monkey-patch the transport methods
        transport.start = async () => {};
        transport.send = async () => {};
        transport.close = async () => {
          if (conn.stdioClient) {
            conn.stdioClient.kill();
          }
        };

        // Store the stdio client in the connection
        conn.stdioClient = stdioClient;
      } else {
        throw new McpConnectionError(
          config.name,
          new Error(`Unsupported server type: ${config.type}`),
        );
      }

      // Setup common transport handlers
      transport.onerror = (err) => {
        const errorMessage = err instanceof Error ? err.message : String(err);
        warn(`Transport Error - ${config.name}: ${errorMessage}`);

        conn.status = "error";
        conn.error = `${
          conn.error ? conn.error + "\n" : ""
        }Transport error: ${errorMessage}`;
      };

      transport.onclose = () => {
        debug(`Transport closed for ${config.name}`);

        // Only mark as disconnected if it wasn't already an error
        if (conn.status !== "error") {
          conn.status = "disconnected";
        }
      };

      // Attempt to connect
      await client.connect(transport);

      // Fetch tools upon successful connection
      let tools: Array<McpToolDefinition> = [];
      try {
        const response = await client.request(
          { method: "tools/list" },
          ListToolsResultSchema,
          { timeout: DEFAULT_REQUEST_TIMEOUT_MS },
        );

        tools = (response?.tools || []).map((sdkTool) => ({
          name: sdkTool.name,
          description: sdkTool.description || "",
          parameters: sdkTool.inputSchema,
        }));

        info(`Fetched ${tools.length} tools from ${config.name}`);
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : String(err);
        warn(`Failed to fetch tools for ${config.name}: ${errorMessage}`);
        // Continue with connection but note the tool fetch failure
      }

      // Update connection state to connected
      conn.client = client;
      conn.transport = transport;
      conn.status = "connected";
      conn.tools = tools;
      conn.error = undefined; // Clear any previous errors

      info(`Successfully connected to ${config.name}`);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      logError(`Connection to ${config.name} failed: ${errorMessage}`);

      // Update connection to reflect error state
      conn.status = "error";
      conn.error = errorMessage;

      // Attempt to close transport if it exists
      if (conn.transport) {
        try {
          await conn.transport.close();
        } catch {
          /* ignore cleanup error */
        }
      }

      // Create a proper error object
      const mcpError =
        err instanceof McpError
          ? err
          : new McpConnectionError(
              config.name,
              err instanceof Error ? err : new Error(errorMessage),
            );

      throw mcpError;
    }
  }

  /**
   * Get all available tools from connected servers, namespaced.
   */
  getAvailableTools(): Array<McpToolDefinition> {
    const allTools: Array<McpToolDefinition> = [];

    this.connections.forEach((conn, serverName) => {
      if (conn.status === "connected" && conn.tools) {
        conn.tools.forEach((tool) => {
          // Namespace the tool name: mcp__serverName__toolName
          allTools.push({
            ...tool,
            name: `mcp__${serverName}__${tool.name}`,
          });
        });
      }
    });

    return allTools;
  }

  /**
   * Execute a tool call on the specified server with improved error handling.
   */
  async callTool(
    serverName: string,
    toolName: string,
    args: Record<string, any>,
  ): Promise<McpToolResult> {
    return executeWithErrorHandling(
      async () => {
        // Check connection status first
        const connection = this.connections.get(serverName);
        if (!connection) {
          const availableServers = Array.from(this.connections.keys()).join(
            ", ",
          );
          throw new McpNotFoundError(
            "server",
            serverName,
            `Available servers: ${availableServers || "none"}`,
          );
        }

        if (connection.status !== "connected") {
          throw new McpConnectionError(
            serverName,
            new Error(
              `Server is not connected (status: ${connection.status})${
                connection.error ? ": " + connection.error : ""
              }`,
            ),
          );
        }

        if (!connection.client) {
          throw new McpConnectionError(
            serverName,
            new Error("No active client available"),
          );
        }

        // TODO: Add server-specific timeout from config if available
        const timeout = DEFAULT_TOOL_EXEC_TIMEOUT_MS;

        debug(
          `Calling tool '${toolName}' on server '${serverName}' with args: ${JSON.stringify(
            args,
          )}`,
        );

        // Use the stdio client if available for better error handling
        if (connection.stdioClient) {
          try {
            const result = await connection.stdioClient.send({
              method: "tools/call",
              params: {
                name: toolName,
                arguments: args,
              },
            });

            debug(
              `Tool '${toolName}' execution completed on '${serverName}' using stdio client`,
            );

            // Process the result
            return this.processToolResult(result);
          } catch (err) {
            // Create a proper tool error
            throw new McpToolError(
              serverName,
              toolName,
              err instanceof Error ? err : new Error(String(err)),
            );
          }
        } else {
          // Fall back to the SDK client if stdio client isn't available
          try {
            const result = await connection.client.request(
              {
                method: "tools/call",
                params: {
                  name: toolName,
                  arguments: args,
                },
              },
              CallToolResultSchema,
              { timeout },
            );

            debug(`Tool '${toolName}' execution completed using SDK client`);

            // Process result consistently
            return this.processToolResult(result);
          } catch (err) {
            throw new McpToolError(
              serverName,
              toolName,
              err instanceof Error ? err : new Error(String(err)),
            );
          }
        }
      },
      (error) => {
        // Convert any error to a consistent McpToolResult
        return { error: error.message };
      },
    );
  }

  /**
   * Consistently process tool results to return a valid McpToolResult
   */
  private processToolResult(result: any): McpToolResult {
    // If the result is undefined, null, or empty object, provide a default success result
    if (
      result === undefined ||
      result === null ||
      (typeof result === "object" && Object.keys(result).length === 0)
    ) {
      return { result: "Tool call completed with no output (assumed success)" };
    }

    // If result matches our expected format, use it directly
    if (
      result &&
      typeof result === "object" &&
      ("result" in result || "error" in result || "partial" in result)
    ) {
      return result as McpToolResult;
    }

    // Otherwise wrap it
    return { result };
  }

  /**
   * Disconnect all clients and clean up resources.
   */
  async dispose(): Promise<void> {
    info("Disposing all connections...");

    this.hardAbort?.abort();

    const closePromises = Array.from(this.connections.values()).map(
      async (conn) => {
        try {
          // Kill stdio client if it exists
          if (conn.stdioClient) {
            conn.stdioClient.kill();
            debug(`Killed stdio client for ${conn.serverConfig.name}`);
          }

          // Close transport if it exists
          if (conn.transport) {
            await conn.transport.close();
          }

          // Close client if it exists
          if (conn.client) {
            await conn.client.close();
          }

          debug(`Closed connection to ${conn.serverConfig.name}`);
        } catch (err) {
          const errorMessage = err instanceof Error ? err.message : String(err);
          warn(
            `Error closing connection to ${conn.serverConfig.name}: ${errorMessage}`,
          );
        }
      },
    );

    await Promise.all(closePromises);
    this.connections.clear();
    this.isInitialized = false;
    info("All connections disposed");
  }
}
