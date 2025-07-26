import type { StdioServerParameters } from "@modelcontextprotocol/sdk/client/stdio.js";
import type { Tool } from "@modelcontextprotocol/sdk/types.js";

import { log } from "../../logger/log";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { SSEClientTransport } from "@modelcontextprotocol/sdk/client/sse.js";
import {
  getDefaultEnvironment,
  StdioClientTransport,
} from "@modelcontextprotocol/sdk/client/stdio.js";
import chalk from "chalk";

type ConnectToServerOptions = StdioServerParameters | { url: string };

export class MCPClient {
  private mcp: Client;
  private transport: StdioClientTransport | SSEClientTransport | null = null;
  private tools: Array<Tool> = [];
  public name: string;

  constructor(name: string, version: string) {
    this.name = name;
    this.mcp = new Client({ name, version });
  }

  async connectToServer(options: ConnectToServerOptions): Promise<void> {
    if (this.isStdioConfig(options)) {
      const { command, args, env } = options;
      try {
        this.transport = new StdioClientTransport({
          command,
          args,
          env: {
            ...getDefaultEnvironment(), // inherit default env vars, base env variables are required for the server to run
            ...env,
          },
          stderr: "pipe",
        });
      } catch (e) {
        // eslint-disable-next-line no-console
        console.error(chalk.red(`Failed to connect to MCP server: ${e}`));
        throw e;
      }
    } else if (this.isSSETransportConfig(options)) {
      const { url } = options;
      try {
        this.transport = new SSEClientTransport(new URL(url));
        log(`Connecting to server with url ${url}`);
      } catch (e) {
        // eslint-disable-next-line no-console
        console.error(chalk.red(`Failed to connect to MCP server: ${e}`));
        throw e;
      }
    } else {
      throw new Error("Invalid config");
    }
    await this.mcp.connect(this.transport);
    const toolsResult = await this.mcp.listTools();
    this.tools = toolsResult.tools.map((tool) => {
      return {
        name: tool.name,
        description: tool.description,
        inputSchema: tool.inputSchema,
      };
    });
    log(
      `Connected to server with tools: ${this.tools.map(({ name }) => name).join(", ")}`,
    );
  }

  async close(): Promise<void> {
    await this.mcp.close();
  }

  async getTools(): Promise<Array<Tool>> {
    return this.tools;
  }

  async callTool(
    toolName: string,
    input: Record<string, unknown>,
  ): Promise<unknown> {
    const tool = this.tools.find((tool) => tool.name === toolName);
    if (!tool) {
      throw new Error(`Tool ${toolName} not found`);
    }
    const result = await this.mcp.callTool({
      name: tool.name,
      arguments: input,
    });
    log(
      `Tool ${toolName} called with input ${JSON.stringify(input)} and result ${JSON.stringify(result)}`,
    );
    return result.content;
  }

  private isStdioConfig(
    config: ConnectToServerOptions,
  ): config is StdioServerParameters {
    return "command" in config;
  }

  private isSSETransportConfig(
    config: ConnectToServerOptions,
  ): config is { url: string } {
    return "url" in config;
  }
}
