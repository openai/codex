import type { Tool } from "@modelcontextprotocol/sdk/types.js";

import { MCPClient } from "./client";
import { getMcpServers } from "../../config";
import { log } from "../../logger/log";
import { flattenTools } from "../utils/flatten";

/**
 * Manages connections to MCP servers and provides access to their tools
 * Implemented as a singleton
 */
export class MCPManager {
  private clients: Map<string, MCPClient> = new Map();
  private connectionStatus: Map<string, boolean> = new Map();
  private isInitializing: boolean = false;
  #isInitialized: boolean = false;

  constructor() {
    // Initialize maps, but don't connect yet
  }

  /**
   * Initialize connections to all configured MCP servers
   * Continues even if some connections fail
   */
  async initialize(): Promise<void> {
    // Skip if already initializing or initialized
    if (this.isInitializing || this.#isInitialized) {
      log("MCPManager: Already initializing or initialized, skipping");
      return;
    }

    this.isInitializing = true;
    const mcpServers = getMcpServers();
    log(
      `MCPManager: Initializing with ${Object.keys(mcpServers).length} servers`,
    );

    // Check if there are any servers to connect to
    if (Object.keys(mcpServers).length === 0) {
      log("MCPManager: No servers configured, marking as initialized");
      this.#isInitialized = true;
      this.isInitializing = false;
      return;
    }

    const mcpServerPromises: Array<Promise<void>> = [];

    // Try to connect to each server in the config
    for (const [serverName, serverConfig] of Object.entries(mcpServers)) {
      try {
        const client = new MCPClient(serverName, "1.0.0");

        if (serverConfig.command) {
          // Local server with command to execute
          const connectToStdioServer = async () => {
            if (!serverConfig.command) {
              // This should never happen as we are already checking for command above
              // this is just to make typescript happy
              throw new Error(`Command not found for server: ${serverName}`);
            }
            try {
              await client.connectToServer({
                command: serverConfig.command,
                args: serverConfig.args || [],
                env: serverConfig.env,
                stderr: 1,
              });
              this.clients.set(serverName, client);
              this.connectionStatus.set(serverName, true);
              log(`Connected to MCP server: ${serverName}`);
            } catch (err) {
              log(`Failed to connect to MCP server ${serverName}: ${err}`);
              this.connectionStatus.set(serverName, false);
            }
          };
          mcpServerPromises.push(connectToStdioServer());
        } else if (serverConfig.url) {
          // Remote server connection
          const connectToRemoteServer = async () => {
            if (!serverConfig.url) {
              // This should never happen as we are already checking for url above
              // this is just to make typescript happy
              throw new Error(`URL not found for server: ${serverName}`);
            }
            try {
              await client.connectToServer({
                url: serverConfig.url,
              });
              this.clients.set(serverName, client);
              this.connectionStatus.set(serverName, true);
              log(`Connected to MCP server: ${serverName}`);
            } catch (err) {
              log(`Failed to connect to MCP server ${serverName}: ${err}`);
              this.connectionStatus.set(serverName, false);
            }
          };
          mcpServerPromises.push(connectToRemoteServer());
        } else {
          log(`Invalid MCP server config for: ${serverName}`);
          this.connectionStatus.set(serverName, false);
        }
      } catch (error) {
        log(`Failed to create MCP client for ${serverName}: ${error}`);
        this.connectionStatus.set(serverName, false);
      }
    }

    try {
      log("MCPManager: Waiting for all connection attempts to finish");
      const results = await Promise.allSettled(mcpServerPromises);
      results.forEach((result, index) => {
        if (result.status === "fulfilled") {
          log(`Connection attempt ${index} completed successfully`);
        } else {
          log(`Connection attempt ${index} failed: ${result.reason}`);
        }
      });
    } catch (err) {
      log(`MCPManager: Error while waiting for connections: ${err}`);
    } finally {
      log("MCPManager: Connection attempts completed");
      this.#isInitialized = true;
      this.isInitializing = false;
    }
  }

  public isInitialized(): boolean {
    return this.#isInitialized;
  }

  public isConnecting(): boolean {
    return this.isInitializing;
  }

  /**
   * Get all successfully connected MCP clients
   */
  getConnectedClients(): Array<MCPClient> {
    const connectedClients: Array<MCPClient> = [];

    for (const [serverName, client] of this.clients.entries()) {
      if (this.connectionStatus.get(serverName)) {
        connectedClients.push(client);
      }
    }

    return connectedClients;
  }

  /**
   * Get a specific MCP client by name
   */
  getClientByName(serverName: string): MCPClient | undefined {
    return this.clients.get(serverName);
  }

  /**
   * Get connection status for a specific server
   */
  isServerConnected(serverName: string): boolean {
    return Boolean(this.connectionStatus.get(serverName));
  }

  /**
   * Get all flattened tools from connected MCP servers
   * Tools are prefixed with server name for uniqueness
   */
  async getFlattendTools(): Promise<Array<Tool>> {
    try {
      const connectedClients = this.getConnectedClients();
      log(
        `MCPManager: Getting tools from ${connectedClients.length} connected clients`,
      );

      // If no clients are connected, return an empty array immediately
      if (connectedClients.length === 0) {
        log("MCPManager: No connected clients, returning empty tools array");
        return [];
      }

      // Use Promise.allSettled to handle failures gracefully
      const tools = await Promise.race([
        flattenTools(connectedClients),
        // Add a timeout to prevent hanging
        new Promise<Array<Tool>>((resolve) => {
          setTimeout(() => {
            log("MCPManager: Tool fetching timed out after 5 seconds");
            resolve([]);
          }, 5000);
        }),
      ]);

      log(`MCPManager: Found ${tools.length} tools`);
      return tools;
    } catch (error) {
      log(`MCPManager: Error fetching tools: ${error}`);
      return [];
    }
  }

  /**
   * Get status of all MCP servers
   */
  getServersStatus(): Array<{ name: string; connected: boolean }> {
    return Array.from(this.connectionStatus.entries()).map(
      ([name, connected]) => ({
        name,
        connected,
      }),
    );
  }

  /**
   * Disconnect all clients gracefully
   */
  async disconnectAll(): Promise<void> {
    const disconnectPromises = [];
    for (const client of this.clients.values()) {
      disconnectPromises.push(client.close());
    }
    await Promise.all(disconnectPromises);
    this.clients.clear();
    this.connectionStatus.clear();
    this.#isInitialized = false;
    log("All MCP clients disconnected");
  }
}
