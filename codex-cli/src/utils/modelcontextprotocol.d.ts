// Type definitions for @modelcontextprotocol/sdk
declare module "@modelcontextprotocol/sdk" {
  export interface MCPTool {
    name: string;
    description: string;
    parameters?: any;
  }

  export interface InitResponse {
    protocol: string;
    tools: Array<{
      name: string;
      description: string;
    }>;
  }

  export interface ClientOptions {
    transport: "stdio" | "sse";
    stdin?: NodeJS.WritableStream;
    stdout?: NodeJS.ReadableStream;
    url?: string;
    childProcess?: any; // For stdio mode with child process
  }

  export class Client {
    constructor(options: ClientOptions);

    initialize(): Promise<InitResponse>;

    listTools(): Promise<Array<MCPTool>>;

    invoke(toolName: string, args: any): Promise<any>;
  }

  export function createMCPClient(options: ClientOptions): Client;
}
