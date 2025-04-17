// This file contains tests for the MCP protocol implementation.

import { describe, expect, test, beforeAll, afterAll } from "vitest";
import type { ChildProcessWithoutNullStreams } from "child_process";
import { spawn } from "child_process";
import path from "path";

// Path to the example server script
const SERVER_SCRIPT = path.resolve(__dirname, "../scripts/mcp-echo-server.js");

// Direct implementation of MCP client for testing
class DirectMCPClient {
  private readline: any;
  private process: ChildProcessWithoutNullStreams;

  constructor(process: ChildProcessWithoutNullStreams) {
    this.process = process;
    this.readline = null; // Will be initialized in setup()
  }

  async setup() {
    // Dynamically import readline to ensure ESM compatibility
    const readline = await import("readline");
    this.readline = readline.createInterface({
      input: this.process.stdout,
      terminal: false,
    });
  }

  private async sendRequest(message: any): Promise<any> {
    return new Promise((resolve, reject) => {
      const messageJSON = JSON.stringify(message);

      // Set up one-time handler for the response
      const onLine = (line: string) => {
        try {
          const response = JSON.parse(line);
          this.readline.removeListener("line", onLine);
          resolve(response);
        } catch (error) {
          reject(new Error(`Invalid JSON response: ${line}`));
        }
      };

      this.readline.once("line", onLine);

      // Send the request
      this.process.stdin.write(messageJSON + "\n");
    });
  }

  async initialize(): Promise<any> {
    return this.sendRequest({ type: "init" });
  }

  async listTools(): Promise<Array<any>> {
    return this.sendRequest({ type: "list_tools" });
  }

  async invoke(tool: string, args: any): Promise<any> {
    return this.sendRequest({ type: "invoke", tool, args });
  }
}

// Full implementation of MCP protocol tests
describe("MCP Protocol", () => {
  let client: DirectMCPClient;
  let serverProcess: ChildProcessWithoutNullStreams;

  beforeAll(async () => {
    // Start the test server process
    serverProcess = spawn(SERVER_SCRIPT, [], {
      stdio: ["pipe", "pipe", "pipe"],
    });

    // Add error handlers for debugging
    serverProcess.on("error", (err) => {
      console.error("Failed to start server process:", err);
    });

    serverProcess.stderr.on("data", (data) => {
      console.log("Server stderr:", data.toString());
    });

    // Create a direct client implementation
    client = new DirectMCPClient(serverProcess);

    // Set up client and wait for server to be ready
    await client.setup();

    // Initialize MCP connection
    await client.initialize();
  }, 20000); // 20 second timeout

  test("list tools", async () => {
    const tools = await client.listTools();
    expect(tools).toBeDefined();
    expect(tools.length).toBeGreaterThan(0);

    // Check if echo tool exists
    const echoTool = tools.find((t) => t.name === "echo");
    expect(echoTool).toBeDefined();
    expect(echoTool?.description).toMatch(/echo/i);
  });

  test("invoke echo tool", async () => {
    const testMessage = "Hello MCP World!";
    const result = await client.invoke("echo", { message: testMessage });
    expect(result.result).toBe(testMessage);
  });

  test("invoke add tool", async () => {
    const a = 5;
    const b = 7;
    const result = await client.invoke("add", { a, b });
    expect(result.result).toBe(a + b);
  });

  // Clean up
  afterAll(() => {
    if (serverProcess) {
      serverProcess.kill();
    }
  });
});
