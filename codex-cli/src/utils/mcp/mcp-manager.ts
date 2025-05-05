import type { Tool } from "@modelcontextprotocol/sdk/types.js";

import { flattenTools } from "./flatten-tools";
import { MCPClient } from "./mcp-client";
import { getMcpServers } from "../config";
import { log } from "../logger/log";

/**
 * Manages connections to MCP servers and provides access to their tools
 */
export class MCPManager {
  private clients: Map<string, MCPClient> = new Map();
  private connectionStatus: Map<string, boolean> = new Map();

  constructor() {
    // Initialize maps, but don't connect yet
  }

  /**
   * Initialize connections to all configured MCP servers
   * Continues even if some connections fail
   */
  async initialize(): Promise<void> {
    const mcpServers = getMcpServers();

    // Try to connect to each server in the config
    for (const [serverName, serverConfig] of Object.entries(mcpServers)) {
      try {
        const client = new MCPClient(serverName, "1.0.0");

        if (serverConfig.command) {
          // Local server with command to execute
          // Note: we're intentionally using await inside a loop
          // because we want to attempt connections serially
          // eslint-disable-next-line no-await-in-loop
          await client.connectToServer(
            serverConfig.command,
            serverConfig.args || [],
            serverConfig.env,
          );
          this.clients.set(serverName, client);
          this.connectionStatus.set(serverName, true);
          log(`Connected to MCP server: ${serverName}`);
        } else if (serverConfig.url) {
          // TODO: Remote server connection (not implemented yet)
          log(`Remote MCP servers not yet supported: ${serverName}`);
          this.connectionStatus.set(serverName, false);
        } else {
          log(`Invalid MCP server config for: ${serverName}`);
          this.connectionStatus.set(serverName, false);
        }
      } catch (error) {
        log(`Failed to connect to MCP server ${serverName}: ${error}`);
        this.connectionStatus.set(serverName, false);
      }
    }
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
    const connectedClients = this.getConnectedClients();
    return flattenTools(connectedClients);
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
  disconnectAll(): void {
    // Currently the MCPClient doesn't have a disconnect method
    // This is a placeholder for future implementation
    this.clients.clear();
    this.connectionStatus.clear();
    log("All MCP clients disconnected");
  }
}
