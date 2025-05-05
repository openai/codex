import type { Tool } from "@modelcontextprotocol/sdk/types.js";

import { log } from "../logger/log";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import chalk from "chalk";

export class MCPClient {
  private mcp: Client;
  private transport: StdioClientTransport | null = null;
  private tools: Array<Tool> = [];
  public name: string;

  constructor(name: string, version: string) {
    this.name = name;
    this.mcp = new Client({ name, version });
  }

  async connectToServer(
    command: string,
    args: Array<string>,
    env?: Record<string, string>,
  ): Promise<void> {
    try {
      this.transport = new StdioClientTransport({
        command,
        args,
        env,
      });
      log(`Connecting to server with command ${command} and args ${args}`);
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
    } catch (e) {
      // eslint-disable-next-line no-console
      console.error(chalk.red(`Failed to connect to MCP server: ${e}`));
      throw e;
    }
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
}
