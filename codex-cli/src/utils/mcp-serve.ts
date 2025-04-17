import fs, { promises as fsPromises } from "fs";
import { createInterface } from "readline";
import express from "express";
import { runSinglePass } from "../cli_singlepass";
import { process_patch } from "./agent/apply-patch";
import { generateFileDiff } from "./singlepass/code_diff";
import { AgentLoop } from "./agent/agent-loop";
import { createInputItem } from "./input-utils";
import { AutoApprovalMode } from "./auto-approval-mode";
import { ReviewDecision } from "./agent/review";
import { MCPTool } from "./mcp";

// Define MCP server options 
export type MCPServeOptions = {
  /** Transport mode: 'stdio' (default) or 'sse' */
  transport?: "stdio" | "sse";
  /** If provided, serve over HTTP on this port */
  port?: number;
};

// Define the tool catalog
const toolCatalog: Array<MCPTool> = [
  {
    name: "echo",
    description: "Echoes the input message (demo tool)",
    parameters: {
      type: "object",
      properties: {
        message: { type: "string" },
      },
      required: ["message"],
    },
  },
  {
    name: "read_file",
    description: "Read a file from the workspace",
    parameters: {
      type: "object",
      properties: {
        path: { type: "string" },
      },
      required: ["path"],
    },
  },
  {
    name: "write_file",
    description: "Write content to a file in the workspace",
    parameters: {
      type: "object",
      properties: {
        path: { type: "string" },
        content: { type: "string" },
      },
      required: ["path", "content"],
    },
  },
  {
    name: "single_pass",
    description: "Execute a one-shot completion with the model",
    parameters: {
      type: "object",
      properties: {
        prompt: { type: "string" },
        model: { type: "string" },
      },
      required: ["prompt"],
    },
  },
  {
    name: "diff",
    description: "Compute diff between two code states",
    parameters: {
      type: "object",
      properties: {
        original: { type: "string" },
        modified: { type: "string" },
        filename: { type: "string" },
      },
      required: ["original", "modified"],
    },
  },
  {
    name: "apply_patch",
    description: "Apply a unified diff to the codebase",
    parameters: {
      type: "object",
      properties: {
        patch: { type: "string" },
      },
      required: ["patch"],
    },
  },
  {
    name: "spawn_agent",
    description: "Launch a new interactive agent session",
    parameters: {
      type: "object",
      properties: {
        prompt: { type: "string" },
        model: { type: "string" },
      },
      required: ["prompt"],
    },
  },
];

/** Serve MCP requests over stdin/stdout using JSON or SSE framing. */
export async function serveConnection(options: MCPServeOptions = {}): Promise<void> {
  const transport = options.transport ?? "stdio";
  const { port } = options;
  // HTTP server mode
  if (port !== undefined) {
    const app = express();
    app.use(express.json());

    // Register routes
    const routes = () => {
      // Client handshake: protocol and tool list
      app.post("/mcp/init", (_req, res) => {
        res.json({
          protocol: "mcp/1.0",
          tools: toolCatalog.map((t) => ({
            name: t.name,
            description: t.description,
          })),
        });
      });

      // List tools
      app.get("/mcp/tools", (_req, res) => {
        res.json(toolCatalog);
      });
      app.post("/mcp/list_tools", (_req, res) => {
        res.json(toolCatalog);
      });

      // Invoke a tool
      // @ts-ignore - Express type issues, but implementation is correct
      app.post("/mcp/invoke", async (req, res) => {
        const { tool, args, stream } = req.body as {
          tool?: string;
          args?: any;
          stream?: boolean;
        };
        if (typeof tool !== "string") {
          return res.status(400).json({ error: "Missing tool name" });
        }
        const meta = toolCatalog.find((t) => t.name === tool);
        if (!meta) {
          return res.status(400).json({ error: `Unknown tool '${tool}'` });
        }

        // Check for SSE (Accept: text/event-stream or stream param)
        const wantsSSE =
          req.headers.accept === "text/event-stream" ||
          req.query["stream"] === "true" ||
          stream === true;

        // Tool dispatcher for all MCP tools
        async function invokeTool(tool: string, args: any) {
          // Helper for setting up SSE headers
          const setupSSE = () => {
            if (wantsSSE) {
              res.setHeader("Content-Type", "text/event-stream");
              res.setHeader("Cache-Control", "no-cache");
              res.setHeader("Connection", "keep-alive");
              res.flushHeaders();
            }
          };

          // Helper to send SSE event
          const sendSSEEvent = (eventType: string, data: any) => {
            if (wantsSSE) {
              res.write(
                `event: ${eventType}\ndata: ${JSON.stringify(data)}\n\n`,
              );
            }
          };

          // Helper to end SSE connection
          const endSSE = () => {
            if (wantsSSE) {
              res.write("event: end\ndata: {}\n\n");
              res.end();
            }
          };

          // Handle SSE error
          const handleSSEError = (error: Error) => {
            if (wantsSSE) {
              setupSSE();
              sendSSEEvent("error", { error: error.message });
              res.end();
              return true;
            }
            return false;
          };

          switch (tool) {
            case "echo":
              // Simulate streaming for demo
              if (wantsSSE) {
                setupSSE();
                sendSSEEvent("message", { chunk: args.message.slice(0, 3) });
                setTimeout(() => {
                  sendSSEEvent("message", { chunk: args.message.slice(3) });
                  endSSE();
                }, 200);
                return;
              }
              return { result: args.message };

            case "read_file":
              try {
                const fileContent = await fsPromises.readFile(
                  args.path,
                  "utf-8",
                );
                if (wantsSSE) {
                  setupSSE();
                  sendSSEEvent("message", { content: fileContent });
                  endSSE();
                  return;
                }
                return { result: fileContent };
              } catch (err: any) {
                if (handleSSEError(err)) {
                  return;
                }
                return { error: err.message };
              }

            case "write_file":
              try {
                const { path: filePath, content } = args;
                await fsPromises.mkdir(require("path").dirname(filePath), {
                  recursive: true,
                });
                await fsPromises.writeFile(filePath, content);
                return { result: `File written to ${filePath}` };
              } catch (err: any) {
                if (handleSSEError(err)) {
                  return;
                }
                return { error: err.message };
              }

            case "single_pass":
              try {
                // Create a promise that will be resolved when the model completion is done
                let completionResult: any = null;
                const completionPromise = new Promise<void>((resolve) => {
                  // Run the singlepass function with a minimal config
                  const config = {
                    apiKey: process.env["OPENAI_API_KEY"] || "",
                    model: args.model || "o3",
                  };

                  // We're going to capture the output from singlepass
                  const originalConsoleLog = console.log;
                  console.log = (...logArgs) => {
                    completionResult = logArgs.join(" ");
                  };

                  runSinglePass({
                    originalPrompt: args.prompt,
                    config: config as any,
                    rootPath: process.cwd(),
                  }).then(() => {
                    // Restore console.log and resolve the promise
                    console.log = originalConsoleLog;
                    resolve();
                  });
                });

                // Wait for the completion to finish
                await completionPromise;

                return { result: completionResult || "Completion finished" };
              } catch (err: any) {
                if (handleSSEError(err)) {
                  return;
                }
                return { error: err.message };
              }

            case "diff":
              try {
                const { original, modified, filename } = args;
                const diff = generateFileDiff(
                  original,
                  modified,
                  filename || "unnamed-file",
                );
                return { result: diff };
              } catch (err: any) {
                if (handleSSEError(err)) {
                  return;
                }
                return { error: err.message };
              }

            case "apply_patch":
              try {
                const { patch } = args;

                // Define filesystem functions
                const openFile = (p: string): string => {
                  return fs.readFileSync(p, "utf8");
                };

                const writeFile = (p: string, content: string): void => {
                  const parent = require("path").dirname(p);
                  if (parent !== ".") {
                    fs.mkdirSync(parent, { recursive: true });
                  }
                  fs.writeFileSync(p, content, "utf8");
                };

                const removeFile = (p: string): void => {
                  fs.unlinkSync(p);
                };

                // Process the patch
                const result = process_patch(
                  patch,
                  openFile,
                  writeFile,
                  removeFile,
                );
                return { result };
              } catch (err: any) {
                if (handleSSEError(err)) {
                  return;
                }
                return { error: err.message };
              }

            case "spawn_agent":
              try {
                // Create a promise that will be resolved when the agent is done
                const agentOutput: Array<string> = [];
                const agentPromise = new Promise<void>((resolve) => {
                  // Create an agent loop with minimal config
                  const agent = new AgentLoop({
                    model: args.model || "o3",
                    config: {
                      apiKey: process.env["OPENAI_API_KEY"] || "",
                      model: args.model || "o3",
                    } as any,
                    instructions: "",
                    approvalPolicy: AutoApprovalMode.SUGGEST,
                    onItem: (item: any) => {
                      if (
                        item.type === "message" &&
                        item.role === "assistant"
                      ) {
                        const text = item.content
                          .filter((c: any) => c.type === "output_text")
                          .map((c: any) => c.text)
                          .join("");
                        agentOutput.push(text);

                        if (wantsSSE) {
                          sendSSEEvent("message", { chunk: text });
                        }
                      }
                    },
                    onLoading: () => {
                      /* ignored */
                    },
                    getCommandConfirmation: (_command: Array<string>) => {
                      return Promise.resolve({
                        review: ReviewDecision.NO_CONTINUE,
                      });
                    },
                    onLastResponseId: () => {
                      if (wantsSSE) {
                        endSSE();
                      }
                      resolve();
                    },
                  });

                  if (wantsSSE) {
                    setupSSE();
                  }

                  // Create input and run the agent
                  createInputItem(args.prompt, []).then((inputItem) => {
                    agent.run([inputItem]).catch((e) => {
                      console.error("Agent error:", e);
                      if (wantsSSE) {
                        sendSSEEvent("error", { error: e.message });
                        res.end();
                      }
                      resolve();
                    });
                  });
                });

                // Wait for the agent to finish
                await agentPromise;

                // If not using SSE, return the collected output
                if (!wantsSSE) {
                  return { result: agentOutput.join("\n") };
                }
                return;
              } catch (err: any) {
                if (handleSSEError(err)) {
                  return;
                }
                return { error: err.message };
              }

            default:
              return { error: `Tool '${tool}' not implemented.` };
          }
        }

        // If streaming, handle in dispatcher
        if (wantsSSE) {
          await invokeTool(tool, args);
          return;
        }

        // Non-streaming: respond with JSON
        const result = await invokeTool(tool, args);
        if (result && result.error) {
          return res.status(400).json(result);
        }
        return res.json(result);
      });
    };

    // Initialize routes
    routes();

    // Create HTTP server
    const server = app.listen(port, () => {
      console.log(`MCP HTTP server listening on port ${port}`);
    });

    // Keep the process running until terminated
    await new Promise<void>((resolve) => {
      process.on("SIGINT", () => {
        server.close(() => resolve());
      });
    });
  }

  // Stdio transport
  const rl = createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false,
  });
  rl.on("close", () => process.exit(0));

  rl.on("line", async (line) => {
    const trimmed = line.trim();
    if (!trimmed) {
      return;
    }
    let msg: any;
    try {
      msg = JSON.parse(trimmed);
    } catch {
      writeResponse({ error: "Invalid JSON" });
      return;
    }
    const { type, tool, args } = msg as {
      type: string;
      tool?: string;
      args?: any;
    };
    switch (type) {
      case "init":
        writeResponse({
          protocol: "mcp/1.0",
          tools: toolCatalog.map((t) => ({
            name: t.name,
            description: t.description,
          })),
        });
        break;
      case "list_tools":
        writeResponse(toolCatalog);
        break;
      case "invoke":
        if (typeof tool !== "string") {
          writeResponse({ error: "Missing tool name" });
          break;
        }
        const meta = toolCatalog.find((t) => t.name === tool);
        if (!meta) {
          writeResponse({ error: `Unknown tool '${tool}'` });
          break;
        }

        try {
          // Handle each tool
          switch (tool) {
            case "echo":
              writeResponse({ result: args.message });
              break;

            case "read_file":
              try {
                const fileContent = await fsPromises.readFile(
                  args.path,
                  "utf-8",
                );
                writeResponse({ result: fileContent });
              } catch (err: any) {
                writeResponse({ error: err.message });
              }
              break;

            case "write_file":
              try {
                const { path: filePath, content } = args;
                await fsPromises.mkdir(require("path").dirname(filePath), {
                  recursive: true,
                });
                await fsPromises.writeFile(filePath, content);
                writeResponse({ result: `File written to ${filePath}` });
              } catch (err: any) {
                writeResponse({ error: err.message });
              }
              break;

            case "single_pass":
              try {
                // Capture console.log output to return as result
                let completionResult: any = null;
                const originalConsoleLog = console.log;
                console.log = (...logArgs) => {
                  completionResult = logArgs.join(" ");
                };

                // Run singlepass with minimal config
                await runSinglePass({
                  originalPrompt: args.prompt,
                  config: {
                    apiKey: process.env["OPENAI_API_KEY"] || "",
                    model: args.model || "o3",
                  } as any,
                  rootPath: process.cwd(),
                });

                // Restore console.log
                console.log = originalConsoleLog;
                writeResponse({
                  result: completionResult || "Completion finished",
                });
              } catch (err: any) {
                writeResponse({ error: err.message });
              }
              break;

            case "diff":
              try {
                const { original, modified, filename } = args;
                const diff = generateFileDiff(
                  original,
                  modified,
                  filename || "unnamed-file",
                );
                writeResponse({ result: diff });
              } catch (err: any) {
                writeResponse({ error: err.message });
              }
              break;

            case "apply_patch":
              try {
                const { patch } = args;

                // Define filesystem functions
                const openFile = (p: string): string => {
                  return fs.readFileSync(p, "utf8");
                };

                const writeFile = (p: string, content: string): void => {
                  const parent = require("path").dirname(p);
                  if (parent !== ".") {
                    fs.mkdirSync(parent, { recursive: true });
                  }
                  fs.writeFileSync(p, content, "utf8");
                };

                const removeFile = (p: string): void => {
                  fs.unlinkSync(p);
                };

                // Process the patch
                const result = process_patch(
                  patch,
                  openFile,
                  writeFile,
                  removeFile,
                );
                writeResponse({ result });
              } catch (err: any) {
                writeResponse({ error: err.message });
              }
              break;

            case "spawn_agent":
              try {
                // Collect agent output
                const agentOutput: Array<string> = [];

                // Create an agent loop with minimal config
                const agent = new AgentLoop({
                  model: args.model || "o3",
                  config: {
                    apiKey: process.env["OPENAI_API_KEY"] || "",
                    model: args.model || "o3",
                  } as any,
                  instructions: "",
                  approvalPolicy: AutoApprovalMode.SUGGEST,
                  onItem: (item: any) => {
                    if (item.type === "message" && item.role === "assistant") {
                      const text = item.content
                        .filter((c: any) => c.type === "output_text")
                        .map((c: any) => c.text)
                        .join("");
                      agentOutput.push(text);

                      // Send each chunk as a partial result
                      writeResponse({ partial: text });
                    }
                  },
                  onLoading: () => {
                    /* ignored */
                  },
                  getCommandConfirmation: (_command: Array<string>) => {
                    return Promise.resolve({
                      review: ReviewDecision.NO_CONTINUE,
                    });
                  },
                  onLastResponseId: () => {
                    // Send final result
                    writeResponse({ result: agentOutput.join("\n") });
                  },
                });

                // Create input and run the agent
                const inputItem = await createInputItem(args.prompt, []);
                await agent.run([inputItem]);
              } catch (err: any) {
                writeResponse({ error: err.message });
              }
              break;

            default:
              writeResponse({ error: `Tool '${tool}' not yet implemented.` });
          }
        } catch (err: any) {
          writeResponse({ error: err.message || String(err) });
        }
        break;

      default:
        writeResponse({ error: `Unknown message type '${type}'` });
    }
  });

  function writeResponse(obj: any) {
    const data = JSON.stringify(obj);
    if (transport === "stdio") {
      process.stdout.write(data + "\n");
    } else {
      process.stdout.write(`event: message\ndata: ${data}\n\n`);
    }
  }

  // Keep running until stdin closes
  await new Promise<void>(() => {});
}

// For backward compatibility, also export serve
export async function serve(options: MCPServeOptions = {}): Promise<void> {
  return serveConnection(options);
}