#!/usr/bin/env -S NODE_OPTIONS=--no-deprecation node

import type { AppRollout } from "./app";
import type { CommandConfirmation } from "./utils/agent/agent-loop";
import type { AppConfig } from "./utils/config";
import type { MCPServer } from "./utils/mcp";
import type { ApprovalPolicy } from "@lib/approvals";
import type { ResponseItem } from "openai/resources/responses/responses";

import App from "./app";
import { runSinglePass } from "./cli_singlepass";
import { AgentLoop } from "./utils/agent/agent-loop";
import { initLogger } from "./utils/agent/log";
import { ReviewDecision } from "./utils/agent/review";
import { AutoApprovalMode } from "./utils/auto-approval-mode";
import { loadConfig, PRETTY_PRINT } from "./utils/config";
import { createInputItem } from "./utils/input-utils";
import {
  listMcpServers,
  addMcpServer,
  removeMcpServer,
} from "./utils/mcp";
import { serve as serveMcp } from "./utils/mcp-serve";
import { McpManager } from "./utils/mcp-manager"; // Use McpManager instead of McpClientRegistry
import {
  isModelSupportedForResponses,
  preloadModels,
} from "./utils/model-utils.js";
import { parseToolCall } from "./utils/parsers";
import { createRobustClient } from "./utils/robust-mcp-cli";
import { onExit, setInkRenderer } from "./utils/terminal";
import chalk from "chalk";
import fs from "fs";
import { render } from "ink";
import meow from "meow";
import path from "path";
import { stdin as input, stdout as output } from "process";
import React from "react";
import { createInterface } from "readline/promises";
// Import removed: spawn is imported but not used in this file

// Call this early so `tail -F "$TMPDIR/oai-codex/codex-cli-latest.log"` works
// immediately. This must be run with DEBUG=1 for logging to work.
initLogger();

// Helper to create an MCP client for a specific server
async function createMcpClientForMcpServer(
  mcpServerName: string,
): Promise<{ client: any; mcpServer: MCPServer }> {
  console.log(`Creating MCP client for MCP Server: ${mcpServerName}`);

  // Check local then global scope
  const localMcpServers = await listMcpServers("local");
  const globalMcpServers = await listMcpServers("global");
  const allMcpServers = [...localMcpServers, ...globalMcpServers];

  // Find the server by name
  const mcpServer = allMcpServers.find((s: MCPServer) => s.name === mcpServerName);
  if (!mcpServer) {
    throw new Error(
      `MCP Server '${mcpServerName}' not found. Use 'codex --mcp list' to see available MCP Servers.`,
    );
  }

  console.log(`Found MCP Server '${mcpServerName}' of type '${mcpServer.type}'`);

  // Create a client based on server type
  let client: any;
  const childProcess: any = null;
  try {
    // For stdio servers, use our robust client implementation
    if (mcpServer.type === "stdio") {
      if (!mcpServer.cmd) {
        throw new Error(
          `MCP Server '${mcpServerName}' is missing required 'cmd' field.`,
        );
      }

      console.log(
        `Using robust client for stdio MCP Server: ${mcpServer.cmd} ${(
          mcpServer.args || []
        ).join(" ")}`,
      );

      try {
        // Create a robust client that handles line-buffered JSON parsing
        client = await createRobustClient(mcpServer);
      } catch (err) {
        if (err instanceof Error) {
          console.error(`Error creating robust MCP client: ${err.message}`);
        } else {
          console.error(`Error creating robust MCP client: ${String(err)}`);
        }
        throw err;
      }
    } else if (mcpServer.type === "sse") {
      if (!mcpServer.url) {
        throw new Error(
          `Server '${mcpServerName}' is missing required 'url' field.`,
        );
      }

      console.log(`Connecting to SSE server at: ${mcpServer.url}`);

      // Simple direct approach for SSE
      // Create a client with SSE support
      try {
        // We'll implement a simplified client that communicates with the SSE server
        const EventSource = (await import("eventsource")) as any;

        // Create a client that uses EventSource for SSE
        const eventSource = new EventSource(mcpServer.url!);

        client = {
          initialize: async () => {
            return new Promise((resolve, reject) => {
              const timeoutId = setTimeout(
                () => reject(new Error("Initialization timed out")),
                5000,
              );

              const onMessage = (event: MessageEvent) => {
                try {
                  const data = JSON.parse(event.data);
                  if (data.protocol) {
                    clearTimeout(timeoutId);
                    eventSource.removeEventListener("message", onMessage);
                    resolve(data);
                  }
                } catch (err) {
                  if (err instanceof Error) {
                    console.error("Failed to parse SSE message:", err.message);
                  } else {
                    console.error("Failed to parse SSE message:", String(err));
                  }
                }
              };

              eventSource.addEventListener("message", onMessage);

              // Send init request
              fetch(`${mcpServer.url}/init`, { method: "POST" }).catch((err) => {
                clearTimeout(timeoutId);
                reject(err);
              });
            });
          },

          listTools: async () => {
            try {
              const response = await fetch(`${mcpServer.url}/list_tools`, {
                method: "POST",
              });
              return await response.json();
            } catch (err) {
              console.error("Error listing tools:", err);
              return [];
            }
          },

          invoke: async (tool: string, args: any) => {
            try {
              const response = await fetch(`${mcpServer.url}/invoke`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ tool, args }),
              });
              return await response.json();
            } catch (err) {
              if (err instanceof Error) {
                return { error: err.message };
              } else {
                return { error: String(err) };
              }
            }
          },
        };
      } catch (err) {
        if (err instanceof Error) {
          console.error(`Error creating SSE client: ${err.message}`);
        } else {
          console.error(`Error creating SSE client: ${String(err)}`);
        }
        throw err;
      }
    } else {
      throw new Error(`Unsupported MCP Server type: ${mcpServer.type}`);
    }

    // Client is already created directly for each transport type
    console.log("Connected to MCP server");
  } catch (err: any) {
    console.error(`Error connecting to MCP server: ${err.message}`);
    console.error("Make sure @modelcontextprotocol/sdk package is installed");

    // Kill the child process if it exists
    if (childProcess) {
      childProcess.kill();
    }
    throw err; // Propagate the error
  }

  return { client, mcpServer };
}

// TODO: migrate to new versions of quiet mode
//
//     -q, --quiet    Non-interactive quiet mode that only prints final message
//     -j, --json     Non-interactive JSON output mode that prints JSON messages

const cli = meow(
  `
  Usage
    $ codex [options] <prompt>

  Options
    -h, --help                 Show usage and exit
    -m, --model <model>        Model to use for completions (default: o3)
    -i, --image <path>         Path(s) to image files to include as input
    -v, --view <rollout>       Inspect a previously saved rollout instead of starting a session
    -q, --quiet                Non-interactive mode that only prints the assistant's final output
    -a, --approval-mode <mode> Override the approval policy: 'suggest', 'auto-edit', or 'full-auto'

    --auto-edit                Automatically approve file edits; still prompt for commands
    --full-auto                Automatically approve edits and commands when executed in the sandbox

    --no-project-doc           Do not automatically include the repository's 'codex.md'
    --project-doc <file>       Include an additional markdown file at <file> as context
    --full-stdout              Do not truncate stdout/stderr from command outputs
    --with-mcp-tools           Enable MCP tools in interactive session (default: true)

  Dangerous options
    --dangerously-auto-approve-everything
                               Skip all confirmation prompts and execute commands without
                               sandboxing. Intended solely for ephemeral local testing.

  Experimental options
    -f, --full-context         Launch in "full-context" mode which loads the entire repository
                               into context and applies a batch of edits in one go. Incompatible
                               with all other flags, except for --model.

  Examples
    $ codex "Write and run a python program that prints ASCII art"
    $ codex -q "fix build issues"
  
  MCP Client commands
    $ codex --mcp add --name myserver --type stdio \
        --cmd /path/to/server --args arg1 arg2 --env KEY=VALUE --scope local
    $ codex --mcp add --name webserver --type sse --url http://localhost:8080 --scope local
    $ codex --mcp list
    $ codex --mcp tools --name myserver
    $ codex --mcp invoke --name myserver --tool echo --payload '{"message":"Hello"}'
  
  MCP Server mode
    $ codex --mcp serve --type stdio    # run codex as an MCP Server over stdio
    $ codex --mcp serve --type sse --port 8080  # run codex as an MCP Server over HTTP
`,
  {
    importMeta: import.meta,
    autoHelp: true,
    flags: {
      // misc
      help: { type: "boolean", aliases: ["h"] },
      view: { type: "string" },
      model: { type: "string", aliases: ["m"] },
      image: { type: "string", isMultiple: true, aliases: ["i"] },
      quiet: {
        type: "boolean",
        aliases: ["q"],
        description: "Non-interactive quiet mode",
      },
      dangerouslyAutoApproveEverything: {
        type: "boolean",
        description:
          "Automatically approve all commands without prompting. This is EXTREMELY DANGEROUS and should only be used in trusted environments.",
      },
      autoEdit: {
        type: "boolean",
        description: "Automatically approve edits; prompt for commands.",
      },
      fullAuto: {
        type: "boolean",
        description:
          "Automatically run commands in a sandbox; only prompt for failures.",
      },
      approvalMode: {
        type: "string",
        aliases: ["a"],
        description:
          "Determine the approval mode for Codex (default: suggest) Values: suggest, auto-edit, full-auto",
      },
      noProjectDoc: {
        type: "boolean",
        description: "Disable automatic inclusion of project‑level codex.md",
      },
      projectDoc: {
        type: "string",
        description: "Path to a markdown file to include as project doc",
      },
      fullStdout: {
        type: "boolean",
        description:
          "Disable truncation of command stdout/stderr messages (show everything)",
        aliases: ["no-truncate"],
      },

      withMcpTools: {
        type: "boolean",
        description: "Enable MCP tools in interactive session",
        default: true,
      },

      mcpDebug: {
        type: "boolean",
        description: "Show detailed MCP debugging information",
        default: false,
      },

      // Experimental mode where whole directory is loaded in context and model is requested
      // to make code edits in a single pass.
      fullContext: {
        type: "boolean",
        aliases: ["f"],
        description: `Run in full-context editing approach. The model is given the whole code 
          directory as context and performs changes in one go without acting.`,
      },
      // MCP launcher
      mcp: {
        type: "string",
        description: "MCP subcommand: add, remove, list, serve",
      },
      // MCP commands
      name: {
        type: "string",
        description: "Name of the MCP Server (for add/remove)",
      },
      type: {
        type: "string",
        description: "Type of the MCP server: 'stdio' or 'sse'",
      },
      cmd: {
        type: "string",
        description: "Command to launch stdio server (for add)",
      },
      args: {
        type: "string",
        isMultiple: true,
        description: "Arguments for the command (for add)",
      },
      url: {
        type: "string",
        description: "URL for sse-type MCP Server (for add)",
      },
      env: {
        type: "string",
        isMultiple: true,
        description: "Environment variables (KEY=VALUE) for server (for add)",
      },
      scope: {
        type: "string",
        description: "Scope for MCP config: 'local' or 'global'",
      },
      port: { type: "number", description: "Port for 'mcp serve' command" },
      // MCP client options
      tool: {
        type: "string",
        description: "Tool name to invoke on the MCP server",
      },
      payload: {
        type: "string",
        description: "JSON string of args for mcp invoke",
      },
    },
  },
);

if (cli.flags.help) {
  cli.showHelp();
}
// ---------------------------------------------------------------------------
// MCP sub-commands (--mcp)
// ---------------------------------------------------------------------------
if (cli.flags.mcp) {
  console.log("Running MCP command...");
  const sub = cli.flags.mcp as string;
  const flags = cli.flags as Record<string, any>;
  const scope = flags["scope"] === "global" ? "global" : "local";
  const rl = createInterface({ input, output });
  console.log(`Executing MCP command: ${sub}`); // Debug output

  // We need to handle MCP commands and then exit to prevent the rest of CLI execution
  try {
    switch (sub) {
      case "add": {
        let { name, type, cmd, url } = flags;
        if (!name) {
          name = await rl.question("Name: ");
        }
        if (!type) {
          type = await rl.question("Type (stdio/sse): ");
        }
        const mcpServer: Record<string, any> = { name, type };
        if (type === "stdio") {
          if (!cmd) {
            cmd = await rl.question("Command: ");
          }
          // If user embedded args in the cmd string, split them out
          let appliedSplit = false;
          if (!flags["args"] && cmd.includes(" ")) {
            const parts = cmd.split(/\s+/);
            cmd = parts.shift()!;
            flags["args"] = parts;
            appliedSplit = true;
          }
          // Prompt for args only if user didn't supply via flags or cmd-split
          if (!appliedSplit && !flags["args"]) {
            const a = await rl.question(
              "Args (space-separated, leave blank for none): ",
            );
            flags["args"] = a ? a.split(/\s+/) : [];
          }
          mcpServer["cmd"] = cmd;
          mcpServer["args"] = flags["args"];
        } else if (type === "sse") {
          if (!url) {
            url = await rl.question("URL: ");
          }
          mcpServer["url"] = url;
        } else {
          console.error("Invalid type. Must be 'stdio' or 'sse'.");
          process.exit(1);
        }
        // Parse any provided environment variables (no prompt)
        const envInput: Array<string> = flags["env"] ?? [];
        const env: Record<string, string> = {};
        for (const e of envInput) {
          const parts = e.split("=");
          const k = parts[0];
          const v = parts[1] || "";
          if (k) {
            env[k] = v;
          }
        }
        mcpServer["env"] = env;
        await addMcpServer(mcpServer as any, scope);
        console.log(`Added mcpServer '${name}' to ${scope} MCP config.`);
        break;
      }
      case "remove": {
        let { name } = flags;
        if (!name) {
          name = await rl.question("Name of mcpServer to remove: ");
        }
        await removeMcpServer(name, scope);
        console.log(`Removed mcpServer '${name}' from ${scope} MCP config.`);
        break;
      }
      case "list": {
        const scopes = flags["scope"] ? [scope] : ["local", "global"];
        for (const s of scopes) {
          const list = await listMcpServers(s as any);
          console.log(`${s} MCP servers:`);
          if (list.length === 0) {
            console.log("  (none)");
          }
          for (const svr of list) {
            console.log(`  - ${svr.name} (${svr.type})`);
          }
        }
        break;
      }
      case "serve": {
        const transport = flags["type"] === "sse" ? "sse" : "stdio";
        // Use port if specified to run HTTP MCP server
        const portOption =
          typeof flags["port"] === "number" ? flags["port"] : undefined;
        await serveMcp({ transport, port: portOption });
        break;
      }
      case "tools": {
        // If a specific server is named, use the MCP client SDK
        if (flags["name"]) {
          try {
            console.log(`Listing tools for mcpServer '${flags["name"]}'...`);
            const { client, mcpServer } = await createMcpClientForMcpServer(
              flags["name"],
            );
            console.log(
              `Connected to server '${flags["name"]}' (${mcpServer.type})`,
            );

            // Initialize and get tools
            console.log("Initializing client...");
            await client.initialize();

            console.log("Fetching available tools...");
            const tools = await client.listTools();

            console.log(
              `Tools for MCP mcpServer '${flags["name"]}' (${mcpServer.type}):`,
            );
            if (tools.length === 0) {
              console.log("  (no tools available)");
            } else {
              for (const tool of tools) {
                console.log(`  - ${tool.name}: ${tool.description}`);

                // Show parameters if available
                if (tool.parameters) {
                  console.log(
                    `    Parameters: ${JSON.stringify(tool.parameters)}`,
                  );
                }
              }
            }
          } catch (err: any) {
            console.error(`Error listing tools: ${err.message}`);
            if (err.stack) {
              console.error(err.stack);
            }
            throw err; // Propagate for central error handling
          }
        } else {
          // No server name provided - just list registered servers
          console.log("Available MCP servers:");

          const scopes = flags["scope"] ? [scope] : ["local", "global"];
          let hasMcpServers = false;

          for (const sc of scopes) {
            const mcpServers = await listMcpServers(sc as any);
            if (mcpServers.length === 0) {
              console.log(`${sc} MCP servers: (none)`);
              continue;
            }

            hasMcpServers = true;
            console.log(`${sc} MCP servers:`);
            for (const svr of mcpServers) {
              console.log(`  - ${svr.name} (${svr.type})`);
              if (svr.type === "stdio" && svr.cmd) {
                console.log(
                  `    Command: ${svr.cmd} ${(svr.args || []).join(" ")}`,
                );
                if (svr.env) {
                  console.log(`    Environment: ${JSON.stringify(svr.env)}`);
                }
              } else if (svr.type === "sse" && svr.url) {
                console.log(`    URL: ${svr.url}`);
              }
              console.log(`    Full config: ${JSON.stringify(svr)}`);
            }
          }

          if (hasMcpServers) {
            console.log("\nTo list tools for a specific mcpServer:");
            console.log("  codex --mcp tools --name <server_name>");
          } else {
            console.log("\nNo MCP servers configured. Add one with:");
            console.log(
              "  codex --mcp add --name <name> --type <stdio|sse> [options]",
            );
          }
        }
        break;
      }
      case "invoke": {
        // Invoke a tool on configured MCP servers
        let toolName = flags["tool"] as string;
        if (!toolName) {
          toolName = await rl.question("Tool name: ");
        }

        // Parse the payload
        let argsObj: any = {};
        if (flags["payload"]) {
          try {
            console.log(`Raw payload: ${flags["payload"]}`);
            // First, try to parse as is
            try {
              argsObj = JSON.parse(flags["payload"] as string);
            } catch (e) {
              // If that fails, try using a simpler approach for common patterns
              const payload = flags["payload"] as string;
              if (payload.includes("message") && payload.includes(":")) {
                const match = payload.match(/message["\s:]+([^"]+)/);
                if (match && match[1]) {
                  argsObj = { message: match[1].replace(/[",}]/g, "") || "" };
                }
              }
            }
            console.log(`Parsed payload: ${JSON.stringify(argsObj)}`);
          } catch (err) {
            const errorMessage =
              (err as Error) instanceof Error ? (err as Error).message : String(err);
            console.error(`Invalid JSON payload: ${errorMessage}`);
            console.error("Using default empty payload");
          }
        } else {
          console.log("No payload provided, using empty object as args");
        }

        // If mcpServer name is provided, use the MCP client SDK
        if (flags["name"]) {
          try {
            console.log(
              `Using MCP SDK client to connect to mcpServer '${flags["name"]}'`,
            );

            // Create client for the specified mcpServer
            const { client, mcpServer } = await createMcpClientForMcpServer(
              flags["name"],
            );
            console.log(
              `Connected to mcpServer '${flags["name"]}' (${mcpServer.type})`,
            );

            // Initialize the client and get tools
            console.log("Initializing MCP client...");
            const init = await client.initialize();
            console.log(`Initialized with protocol: ${init.protocol}`);

            // Invoke the tool
            console.log(
              `Invoking tool '${toolName}' with args: ${JSON.stringify(
                argsObj,
              )}`,
            );
            const result = await client.invoke(toolName, argsObj);
            console.log("Result:", JSON.stringify(result, null, 2));
          } catch (err: any) {
            console.error(`Error invoking tool: ${err.message}`);
            if (err.stack) {
              console.error(err.stack);
            }
            throw err; // Propagate for central error handling
          }
          break;
        }

        // No mcpServer name provided - error out
        console.error("Error: mcpServer name is required for invoke command.");
        console.error(
          'Usage: codex --mcp invoke --name <server_name> --tool <tool_name> [--payload \'{"key":"value"}\']',
        );
        process.exit(1);
        break;
      }
      default:
        console.error(
          "Usage: codex --mcp <add|remove|list|serve|tools|invoke> [options]",
        );
    }

    // Successfully completed the MCP command
    console.log("MCP command completed successfully");
  } catch (err: any) {
    console.error(`MCP error: ${err.message || err}`);
    process.exit(1); // Exit with error code
  } finally {
    // Always close readline interface and exit
    rl.close();
    process.exit(0);
  }
}

// ---------------------------------------------------------------------------
// API key handling
// ---------------------------------------------------------------------------

const apiKey = process.env["OPENAI_API_KEY"];

if (!apiKey) {
  // eslint-disable-next-line no-console
  console.error(
    `\n${chalk.red("Missing OpenAI API key.")}\n\n` +
      `Set the environment variable ${chalk.bold("OPENAI_API_KEY")} ` +
      "and re-run this command.\n" +
      `You can create a key here: ${chalk.bold(
        chalk.underline("https://platform.openai.com/account/api-keys"),
      )}\n`,
  );
  process.exit(1);
}

const fullContextMode = Boolean(cli.flags.fullContext);
let config = loadConfig(undefined, undefined, {
  cwd: process.cwd(),
  disableProjectDoc: Boolean(cli.flags.noProjectDoc),
  projectDocPath: cli.flags.projectDoc as string | undefined,
  isFullContext: fullContextMode,
});

const prompt = cli.input[0];
const model = cli.flags.model;
const imagePaths = cli.flags.image as Array<string> | undefined;

config = {
  apiKey,
  ...config,
  model: model ?? config.model,
};

if (!(await isModelSupportedForResponses(config.model))) {
  // eslint-disable-next-line no-console
  console.error(
    `The model "${config.model}" does not appear in the list of models ` +
      "available to your account. Double‑check the spelling (use\n" +
      "  openai models list\n" +
      "to see the full list) or choose another model with the --model flag.",
  );
  process.exit(1);
}

let rollout: AppRollout | undefined;

if (cli.flags.view) {
  const viewPath = cli.flags.view as string;
  const absolutePath = path.isAbsolute(viewPath)
    ? viewPath
    : path.join(process.cwd(), viewPath);
  try {
    const content = fs.readFileSync(absolutePath, "utf-8");
    rollout = JSON.parse(content) as AppRollout;
  } catch (error) {
    // eslint-disable-next-line no-console
    console.error("Error reading rollout file:", error);
    process.exit(1);
  }
}

// If we are running in --fullcontext mode, do that and exit.
if (fullContextMode) {
  await runSinglePass({
    originalPrompt: prompt,
    config,
    rootPath: process.cwd(),
  });
  onExit();
  process.exit(0);
}

// If we are running in --quiet mode, do that and exit.
const quietMode = Boolean(cli.flags.quiet);
const autoApproveEverything = Boolean(
  cli.flags.dangerouslyAutoApproveEverything,
);
const fullStdout = Boolean(cli.flags.fullStdout);

if (quietMode) {
  process.env["CODEX_QUIET_MODE"] = "1";
  if (!prompt || (typeof prompt === "string" && prompt.trim() === "")) {
    // eslint-disable-next-line no-console
    console.error(
      'Quiet mode requires a prompt string, e.g.,: codex -q "Fix bug #123 in the foobar project"',
    );
    process.exit(1);
  }
  await runQuietMode({
    prompt: prompt as string,
    imagePaths: imagePaths || [],
    approvalPolicy: autoApproveEverything
      ? AutoApprovalMode.FULL_AUTO
      : AutoApprovalMode.SUGGEST,
    config,
  });
  onExit();
  process.exit(0);
}

// Default to the "suggest" policy.
// Determine the approval policy to use in interactive mode.
//
// Priority (highest → lowest):
// 1. --fullAuto – run everything automatically in a sandbox.
// 2. --dangerouslyAutoApproveEverything – run everything **without** a sandbox
//    or prompts.  This is intended for completely trusted environments.  Since
//    it is more dangerous than --fullAuto we deliberately give it lower
//    priority so a user specifying both flags still gets the safer behaviour.
// 3. --autoEdit – automatically approve edits, but prompt for commands.
// 4. Default – suggest mode (prompt for everything).

const approvalPolicy: ApprovalPolicy =
  cli.flags.fullAuto || cli.flags.approvalMode === "full-auto"
    ? AutoApprovalMode.FULL_AUTO
    : cli.flags.autoEdit
    ? AutoApprovalMode.AUTO_EDIT
    : AutoApprovalMode.SUGGEST;

preloadModels();

const withMcpTools = cli.flags.withMcpTools !== false; // Default to true unless explicitly set to false
const mcpDebug = Boolean(cli.flags.mcpDebug); // Debug flag for MCP operations

// Set environment variable for MCP debug mode
if (mcpDebug) {
  process.env["MCP_DEBUG"] = "1";
} else {
  delete process.env["MCP_DEBUG"];
}

const instance = render(
  <App
    prompt={prompt}
    config={config}
    rollout={rollout}
    imagePaths={imagePaths}
    approvalPolicy={approvalPolicy}
    fullStdout={fullStdout}
    withMcpTools={withMcpTools}
  />,
  {
    patchConsole: process.env["DEBUG"] ? false : true,
  },
);
setInkRenderer(instance);

function formatResponseItemForQuietMode(item: ResponseItem): string {
  if (!PRETTY_PRINT) {
    return JSON.stringify(item);
  }
  switch (item.type) {
    case "message": {
      const role = item.role === "assistant" ? "assistant" : item.role;
      const txt = item.content
        .map((c) => {
          if (c.type === "output_text" || c.type === "input_text") {
            return c.text;
          }
          if (c.type === "input_image") {
            return "<Image>";
          }
          if (c.type === "input_file") {
            return c.filename;
          }
          if (c.type === "refusal") {
            return c.refusal;
          }
          return "?";
        })
        .join(" ");
      return `${role}: ${txt}`;
    }
    case "function_call": {
      const details = parseToolCall(item);
      return `$ ${details?.cmdReadableText ?? item.name}`;
    }
    case "function_call_output": {
      // @ts-expect-error metadata unknown on ResponseFunctionToolCallOutputItem
      const meta = item.metadata as ExecOutputMetadata;
      const parts: Array<string> = [];
      if (typeof meta?.exit_code === "number") {
        parts.push(`code: ${meta.exit_code}`);
      }
      if (typeof meta?.duration_seconds === "number") {
        parts.push(`duration: ${meta.duration_seconds}s`);
      }
      const header = parts.length > 0 ? ` (${parts.join(", ")})` : "";
      return `command.stdout${header}\n${item.output}`;
    }
    default: {
      return JSON.stringify(item);
    }
  }
}

async function runQuietMode({
  prompt,
  imagePaths,
  approvalPolicy,
  config,
}: {
  prompt: string;
  imagePaths: Array<string>;
  approvalPolicy: ApprovalPolicy;
  config: AppConfig;
}): Promise<void> {
  // Initialize MCP Manager
  const mcpManager = new McpManager({
    debugMode: Boolean(process.env["MCP_DEBUG"]),
  });
  try {
    // Add a timeout to prevent hanging during initialization
    const initPromise = mcpManager.initialize();
    const timeoutPromise = new Promise((_, reject) => {
      setTimeout(
        () =>
          reject(new Error("MCP initialization timed out after 10 seconds")),
        10000,
      );
    });

    await Promise.race([initPromise, timeoutPromise]);
    console.log(
      `MCP Manager initialized with ${
        mcpManager.getAvailableTools().length
      } tools`,
    );
  } catch (err) {
    console.warn(`Warning: Failed to initialize Mcp manager: ${err}`);
    // Continue without MCP functionality
  }

  const agent = new AgentLoop({
    model: config.model,
    config: config,
    instructions: config.instructions,
    approvalPolicy,
    onItem: (item: ResponseItem) => {
      // eslint-disable-next-line no-console
      console.log(formatResponseItemForQuietMode(item));
    },
    onLoading: () => {
      /* intentionally ignored in quiet mode */
    },
    onToolCall: (result) => {
      // Log tool call results
      console.log(`Tool call: ${result.toolCall.name}`);
      if (result.error) {
        console.log(`Error: ${result.error}`);
      } else {
        console.log(`Result: ${JSON.stringify(result.result)}`);
      }
    },
    getCommandConfirmation: (
      _command: Array<string>,
    ): Promise<CommandConfirmation> => {
      return Promise.resolve({ review: ReviewDecision.NO_CONTINUE });
    },
    onLastResponseId: () => {
      /* intentionally ignored in quiet mode */
    },
  });

  const inputItem = await createInputItem(prompt, imagePaths);
  await agent.run([inputItem]);
}

const exit = () => {
  onExit();
  process.exit(0);
};

process.on("SIGINT", exit);
process.on("SIGQUIT", exit);
process.on("SIGTERM", exit);

// ---------------------------------------------------------------------------
// Fallback for Ctrl‑C when stdin is in raw‑mode
// ---------------------------------------------------------------------------

if (process.stdin.isTTY) {
  // Ensure we do not leave the terminal in raw mode if the user presses
  // Ctrl‑C while some other component has focus and Ink is intercepting
  // input. Node does *not* emit a SIGINT in raw‑mode, so we listen for the
  // corresponding byte (0x03) ourselves and trigger a graceful shutdown.
  const onRawData = (data: Buffer | string): void => {
    const str = Buffer.isBuffer(data) ? data.toString("utf8") : data;
    if (str === "\u0003") {
      exit();
    }
  };
  process.stdin.on("data", onRawData);
}

// Ensure terminal clean‑up always runs, even when other code calls
// `process.exit()` directly.
process.once("exit", onExit);
