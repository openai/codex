import type { MCPServer as McpServerConfig } from "./mcp";

import { log } from "./agent/log";
import { listMcpServers } from "./mcp";
// Removed: import { RobustStdioMcpClient } from './robust-mcp-client';
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { SSEClientTransport } from "@modelcontextprotocol/sdk/client/sse.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import {
  CallToolResultSchema,
  ListToolsResultSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { serveConnection } from "./mcp-serve";

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
    log(`[MCP Manager] Error: ${error.message}`);
    return errorHandler(error);
  }
}

// Define internal connection state
type McpConnection = {
  mcpServerConfig: McpServerConfig; // Configuration loaded from file
  client: Client | null; // Allow null client for error states
  transport: StdioClientTransport | SSEClientTransport | null; // Allow null transport
  // Removed: robustClient?: RobustStdioMcpClient; // For robust stdio handling
  status: "connecting" | "connected" | "disconnected" | "error";
  error?: string;
  tools: Array<McpToolDefinition>; // Cache fetched tools
};

// Define the structure for tools provided by MCP servers (matching mcp-client-registry)
export interface McpToolDefinition {
  name: string;
  description: string;
  parameters: any; // JSON Schema object
}

// Define the structure for the result of calling an MCP tool
export interface McpToolResult {
  result?: any;
  error?: string;
  partial?: any; // For streaming results
}

// Timeout for internal MCP requests (e.g., listing tools)
const DEFAULT_REQUEST_TIMEOUT_MS = 5000;
// Default timeout for tool execution (can be overridden bymcpServer config later)
const DEFAULT_TOOL_EXEC_TIMEOUT_MS = 30000;
// Timeout for stdiomcpServer readiness check
const DEFAULT_READINESS_TIMEOUT_MS = 5000;

export class McpManager {
  private connections: Map<string, McpConnection> = new Map();
  private isInitialized = false;
  private clientVersion: string = "1.0.0"; // TODO: Get actual CLI version
  private debugMode = false;

  /**
   * Create a new MCP Manager instance
   * @param options Configuration options
   */
  constructor(options?: { debugMode?: boolean }) {
    // TODO: Add clientVersion retrieval
    this.debugMode = options?.debugMode || false;

    // Check for debug environment variable
    if (process.env["MCP_DEBUG"] === "1") {
      this.debugMode = true;
      log("[MCP Manager] Debug mode enabled via environment variable");
    }
  }

  /**
   * Enable or disable debug mode
   * @param enabled Whether debug mode should be enabled
   */
  public enableDebugMode(enabled: boolean): void {
    this.debugMode = enabled;
    log(`[MCP Manager] Debug mode ${enabled ? "enabled" : "disabled"}`);
  }

  /**
   * Initialize connections to all configured MCP servers.
   */
  async initialize(): Promise<void> {
    if (this.isInitialized) {
      return;
    }
    log("[MCP Manager] Initializing...");

    try {
      const localMcpServers = await listMcpServers("local");
      const globalMcpServers = await listMcpServers("global");
      const allMcpServerConfigs = [...localMcpServers, ...globalMcpServers];

      // Deduplicate based on name, prioritizing local config
      const uniqueMcpServerConfigs = new Map<string, McpServerConfig>();
      for (const mcpServer of allMcpServerConfigs) {
        if (!uniqueMcpServerConfigs.has(mcpServer.name)) {
          uniqueMcpServerConfigs.set(mcpServer.name, mcpServer);
        }
      }

      if (uniqueMcpServerConfigs.size === 0) {
        log("[MCP Manager] No MCP servers configured.");
        this.isInitialized = true;
        return;
      }

      log(
        `[MCP Manager] Found ${uniqueMcpServerConfigs.size} servers. Connecting...`,
      );

      const connectionPromises = Array.from(uniqueMcpServerConfigs.values()).map(
        (config) =>
          this.connectToServer(config).catch((err) => {
            log(
              `[MCP Manager] Failed to connect tomcpServer ${config.name}: ${err}`,
            );
            // Store error state even if initial connection fails
            this.connections.set(config.name, {
              mcpServerConfig: config,
              client: null, // No client if connection failed
              transport: null, // No transport
              status: "error",
              error: err instanceof Error ? err.message : String(err),
              tools: [],
            });
          }),
      );

      // Add timeout for the overall connection process
      const allConnectionsPromise = Promise.all(connectionPromises);
      const timeoutPromise = new Promise(
        (_, reject) =>
          setTimeout(
            () =>
              reject(
                new Error(
                  "MCP initialization timed out waiting formcpServer connections.",
                ),
              ),
            DEFAULT_REQUEST_TIMEOUT_MS + 1000,
          ), // Slightly longer than individual timeouts
      );

      await Promise.race([allConnectionsPromise, timeoutPromise]);

      this.isInitialized = true;
      log(
        `[MCP Manager] Initialization complete. ${this.connections.size} connections attempted.`,
      );
      this.logConnectionStatus();
    } catch (error) {
      log(`[MCP Manager] Error during initialization: ${error}`);
      // Still mark as initialized to prevent retries, but log the failure
      this.isInitialized = true;
      this.logConnectionStatus(); // Log status even on error
    }
  }

  private logConnectionStatus() {
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
    log(
      `[MCP Manager] Status: ${connectedCount} connected, ${errorCount} failed.`,
    );

    // Log detailed status in debug mode
    if (this.debugMode) {
      this.connections.forEach((conn, name) => {
        log(
          `[MCP Manager]mcpServer '${name}': status=${conn.status}, tools=${
            conn.tools.length
          }${conn.error ? `, error=${conn.error}` : ""}`,
        );
      });
    }
  }

  /**
   * Connect to a single MCP Server based on its configuration.
   */
  private async connectToServer(config: McpServerConfig): Promise<void> {
    log(`[MCP Manager] Connecting to ${config.name} (${config.type})...`);
    // Prevent duplicate connections
    if (
      this.connections.has(config.name) &&
      this.connections.get(config.name)?.status !== "disconnected" &&
      this.connections.get(config.name)?.status !== "error"
    ) {
      log(
        `[MCP Manager]mcpServer ${config.name} already connecting or connected.`,
      );
      return;
    }

    // Mark as connecting immediately
    this.connections.set(config.name, {
      mcpServerConfig: config,
      client: null,
      transport: null,
      status: "connecting",
      tools: [],
    });

    let client: Client | null = null;
    let transport: StdioClientTransport | SSEClientTransport | null = null;

    try {
      client = new Client(
        { name: "CodexCLI-Agent", version: this.clientVersion },
        { capabilities: {} },
      );

      if (config.type === "sse") {
        if (!config.url) {
          throw new Error(`SSEmcpServer ${config.name} is missing 'url'.`);
        }
        transport = new SSEClientTransport(new URL(config.url), {});
        // SSE transport doesn't need manual start or readiness check like stdio
      } else if (config.type === "stdio") {
        if (!config.cmd) {
          throw new Error(`StdiomcpServer ${config.name} is missing 'cmd'.`);
        }

        // Use the SDK's StdioClientTransport directly
        log(`[MCP Manager] Using SDK StdioClientTransport for ${config.name}`);
        const stdioTransport = new StdioClientTransport({
          // Create with specific type
          command: config.cmd,
          args: config.args,
          env: Object.fromEntries(
            Object.entries({ ...process.env, ...(config.env || {}) }).filter(
              ([, v]) => v !== undefined,
            ) as Array<[string, string]>,
          ), // Merge env vars
          stderr: "pipe", // Capture stderr
        });
        transport = stdioTransport; // Assign to the broader type variable

        // Start transport manually to capture stderr early and check readiness
        try {
          const startPromise = stdioTransport.start();
          const startTimeout = new Promise((_, reject) =>
            setTimeout(
              () =>
                reject(
                  new Error(`Timeout starting transport for ${config.name}`),
                ),
              DEFAULT_READINESS_TIMEOUT_MS,
            ),
          );
          await Promise.race([startPromise, startTimeout]);
        } catch (startError) {
          log(
            `[MCP Manager] Error starting transport for ${config.name}: ${startError}`,
          );
          throw startError; // Propagate start error
        }

        // Implement readiness detection
        const readyPattern = /server running|ready|started/i;
        let isReady = false;

        const readyPromise = new Promise<void>((resolve) => {
          let timeoutId: NodeJS.Timeout | undefined;
          let stderrListener: ((data: Buffer) => void) | undefined;

          const cleanup = () => {
            if (timeoutId) {
              clearTimeout(timeoutId);
            }
            if (stderrListener && stdioTransport.stderr) {
              stdioTransport.stderr.off("data", stderrListener);
            }
          };

          timeoutId = setTimeout(() => {
            log(
              `[MCP Manager] Timeout waiting for ${config.name} to be ready. Assuming ready.`,
            );
            cleanup();
            resolve(); // Assume ready on timeout
          }, DEFAULT_READINESS_TIMEOUT_MS);

          stderrListener = (data: Buffer) => {
            const output = data.toString().trim();
            if (output) {
              log(`[MCP Stderr - ${config.name}] ${output}`);
              const conn = this.connections.get(config.name);
              if (conn) {
                conn.error = `${
                  conn.error ? conn.error + "\n" : ""
                }stderr: ${output}`;
              }
              if (!isReady && readyPattern.test(output)) {
                isReady = true;
                log(`[MCP Manager]mcpServer ${config.name} signaled ready.`);
                cleanup();
                resolve();
              }
            }
          };

          if (stdioTransport.stderr) {
            stdioTransport.stderr.on("data", stderrListener);
          } else {
            log(
              `[MCP Manager] No stderr stream available for ${config.name}, assuming ready.`,
            );
            cleanup();
            resolve();
          }
          // The main onerror/onclose handlers set later will catch issues
          // if the transport fails before client.connect is called.
        });

        // Wait for readiness check (or timeout)
        await readyPromise;
        // We don't need a try/catch here anymore as the promise only resolves.

        // Monkey-patch start to prevent client.connect from restarting the transport
        stdioTransport.start = async () => {};
      } else {
        throw new Error(`UnsupportedmcpServer type: ${config.type}`);
      }

      if (!transport) {
        throw new Error(
          `Transport could not be initialized for ${config.name}`,
        );
      }

      // Setup common transport handlers (AFTER transport is created)
      transport.onerror = (error) => {
        log(`[MCP Transport Error - ${config.name}] ${error}`);
        const conn = this.connections.get(config.name);
        if (conn) {
          conn.status = "error";
          conn.error = `${
            conn.error ? conn.error + "\n" : ""
          }Transport error: ${
            error instanceof Error ? error.message : String(error)
          }`;
        }
      };
      transport.onclose = () => {
        log(`[MCP Transport Close - ${config.name}]`);
        const conn = this.connections.get(config.name);
        // Only mark as disconnected if it wasn't already an error
        if (conn && conn.status !== "error") {
          conn.status = "disconnected";
        }
      };

      // Attempt to connect
      if (!client) {
        throw new Error("Client not initialized");
      } // Should not happen
      await client.connect(transport);

      // Fetch tools upon successful connection
      let tools: Array<McpToolDefinition> = [];
      try {
        const listToolsPromise = client.request(
          { method: "tools/list" },
          ListToolsResultSchema,
          { timeout: DEFAULT_REQUEST_TIMEOUT_MS },
        );
        const listTimeoutPromise = new Promise((_, reject) =>
          setTimeout(
            () =>
              reject(new Error(`Timeout fetching tools for ${config.name}`)),
            DEFAULT_REQUEST_TIMEOUT_MS,
          ),
        );

        const response = (await Promise.race([
          listToolsPromise,
          listTimeoutPromise,
        ])) as {
          tools?: Array<{
            name: string;
            description?: string;
            inputSchema: any;
          }>;
        };

        tools = (response?.tools || []).map((sdkTool) => ({
          name: sdkTool.name,
          description: sdkTool.description || "", // Ensure description is a string
          parameters: sdkTool.inputSchema, // Map inputSchema to parameters
        }));
        log(`[MCP Manager] Fetched ${tools.length} tools from ${config.name}.`);
      } catch (toolError) {
        log(
          `[MCP Manager] Failed to fetch tools for ${config.name}: ${toolError}`,
        );
        // Proceed with connection but mark tools as empty/failed
      }

      // Update connection state to connected
      this.connections.set(config.name, {
        mcpServerConfig: config,
        client,
        transport,
        status: "connected",
        tools: tools,
      });
      log(`[MCP Manager] Successfully connected to ${config.name}.`);
    } catch (error) {
      log(`[MCP Manager] Connection to ${config.name} failed: ${error}`);
      // Ensure state reflects the error
      const existingConn = this.connections.get(config.name);
      this.connections.set(config.name, {
        ...(existingConn || { mcpServerConfig: config, tools: [] }), // Keep existing config/tools if possible
        client: null, // Set client to null on error
        transport: transport || existingConn?.transport || null, // Keep transport if created
        status: "error",
        error: error instanceof Error ? error.message : String(error),
      });
      // Attempt to close transport if it exists and wasn't the source of the error state already
      if (transport && this.connections.get(config.name)?.status !== "error") {
        try {
          await transport.close();
        } catch {
          /* ignore cleanup error */
        }
      }
      // Do not re-throw here, let initialize handle reporting
    }
  }

  /**
   * Get all available tools from connected servers, namespaced.
   */
  getAvailableTools(): Array<McpToolDefinition> {
    const allTools: Array<McpToolDefinition> = [];
    this.connections.forEach((conn, mcpServerName) => {
      if (conn.status === "connected" && conn.tools) {
        conn.tools.forEach((tool) => {
          // Namespace the tool name: mcp__mcpServerName__toolName
          allTools.push({
            ...tool,
            name: `mcp__${mcpServerName}__${tool.name}`,
          });
        });
      }
    });
    return allTools;
  }

  /**
   * Process tool results consistently to return a valid McpToolResult
   * @param result The raw result from the tool call
   * @returns A standardized McpToolResult object
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

    // If it has content (MCP format), extract the text
    if (
      result &&
      typeof result === "object" &&
      "content" in result &&
      Array.isArray(result.content)
    ) {
      const textItems = result.content
        .filter((item: any) => item.type === "text")
        .map((item: any) => item.text);

      if (textItems.length > 0) {
        return { result: textItems.join("\n") };
      }
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
   * Execute a tool call on the specifiedmcpServer with improved error handling.
   * @param mcpServerName The name of the MCP Server to call the tool on
   * @param toolName The name of the tool to call
   * @param args The arguments to pass to the tool
   * @returns A promise that resolves to the tool result or error
   */
  async callTool(
    mcpServerName: string,
    toolName: string,
    args: Record<string, any>,
  ): Promise<McpToolResult> {
    return executeWithErrorHandling(
      async () => {
        const connection = this.connections.get(mcpServerName);

        if (!connection) {
          return { error: `mcpServer '${mcpServerName}' not found.` };
        }
        if (connection.status !== "connected") {
          return {
            error: `mcpServer '${mcpServerName}' is not connected (status: ${
              connection.status
            }). ${connection.error || ""}`.trim(),
          };
        }
        if (!connection.client) {
          return { error: `mcpServer '${mcpServerName}' has no active client.` };
        }

        // TODO: Add server-specific timeout from config if available
        const timeout = DEFAULT_TOOL_EXEC_TIMEOUT_MS;

        log(
          `[MCP Manager] Calling tool '${toolName}' on mcpServer '${mcpServerName}' with args: ${JSON.stringify(
            args,
          )}`,
        );

        // Use the SDK client directly
        const result = await connection.client.request(
          {
            method: "tools/call",
            params: {
              name: toolName,
              arguments: args,
            },
          },
          CallToolResultSchema, // Validate the response structure
          { timeout },
        );

        if (this.debugMode) {
          console.log(
            `[MCP DEBUG] Raw tool call result for '${toolName}' on mcpServer '${mcpServerName}':`,
            result,
          );
        }

        log(
          `[MCP Manager] Tool '${toolName}' on mcpServer '${mcpServerName}' result received`,
        );

        return this.processToolResult(result);
      },
      (error) => {
        // Log detailed error information in debug mode
        if (this.debugMode && error && typeof error === "object") {
          const errObj = error as any;
          if (errObj.response) {
            log(
              `[MCP Manager] Raw error response: ${JSON.stringify(
                errObj.response,
              )}`,
            );
          }
          if (errObj.stdout) {
            log(`[MCP Manager] Raw error stdout: ${errObj.stdout}`);
          }
          if (errObj.stderr) {
            log(`[MCP Manager] Raw error stderr: ${errObj.stderr}`);
          }
        }

        // Convert any error to a consistent McpToolResult
        return { error: error.message };
      },
    );
  }

  /**
   * Disconnect all clients and clean up resources.
   */
  async dispose(): Promise<void> {
    log("[MCP Manager] Disposing all connections...");
    const closePromises = Array.from(this.connections.values()).map(
      async (conn) => {
        // Removed: Robust client cleanup
        // Close transport if it exists
        if (conn.transport) {
          try {
            await conn.transport.close();
          } catch (e) {
            log(
              `[MCP Manager] Error closing transport for ${conn.mcpServerConfig.name}: ${e}`,
            );
          }
        }

        // Close client if it exists
        if (conn.client) {
          try {
            await conn.client.close();
          } catch (e) {
            log(
              `[MCP Manager] Error closing client for ${conn.mcpServerConfig.name}: ${e}`,
            );
          }
        }
      },
    );
    await Promise.all(closePromises);
    this.connections.clear();
    this.isInitialized = false;
    log("[MCP Manager] Disposed.");
  }

  /**
   * Start an HTTPmcpServer to expose MCP functionality
   * @param optionsmcpServer options including port
   * @returns A promise that resolves when themcpServer is started
   */
  async serve(options: { port: number }): Promise<void> {
    log(`[MCP Manager] Starting HTTPmcpServer on port ${options.port}...`);

    // Use the serveConnection function from mcp-serve.ts
    return serveConnection({
      transport: "sse",
      port: options.port,
    });
  }
}
