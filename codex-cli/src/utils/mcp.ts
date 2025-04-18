import fs, { promises as fsPromises } from "fs";
import os from "os";
import path from "path";

// Types for MCP Server definitions
export type MCPServer = {
  name: string;
  type: "stdio" | "sse";
  // for stdio type
  cmd?: string;
  args?: Array<string>;
  // for sse type
  url?: string;
  // common
  env?: Record<string, string>;
};

type MCPConfig = {
  mcpServers: Record<string, {
    command: string;
    args?: Array<string>;
    env?: Record<string, string>;
  }>;
};

// Config file paths
const GLOBAL_DIR = path.join(os.homedir(), ".codex");
const GLOBAL_FILE = path.join(GLOBAL_DIR, "mcp.json");

// Find project root by locating .git directory
function findProjectRoot(startDir: string): string | null {
  let dir = path.resolve(startDir);
  while (true) {
    if (fs.existsSync(path.join(dir, ".git"))) {
      return dir;
    }
    const parent = path.dirname(dir);
    if (parent === dir) {
      break;
    }
    dir = parent;
  }
  return null;
}

function getLocalFile(): string | null {
  const root = findProjectRoot(process.cwd());
  if (!root) {
    return null;
  }
  const dir = path.join(root, ".codex");
  return path.join(dir, "mcp.json");
}

async function loadConfig(scope: "global" | "local"): Promise<MCPConfig> {
  const file = scope === "global" ? GLOBAL_FILE : getLocalFile();
  if (!file) {
    return { mcpServers: {} };
  }
  try {
    const raw = await fsPromises.readFile(file, "utf-8");
    const parsed = JSON.parse(raw);

    // Support new mcpServers object format
    if (parsed.mcpServers && typeof parsed.mcpServers === "object") {
      return { mcpServers: parsed.mcpServers };
    }

    // Fallback to empty object if not present
    return { mcpServers: {} };
  } catch {
    return { mcpServers: {} };
  }
}

async function saveConfig(
  scope: "global" | "local",
  config: MCPConfig,
): Promise<void> {
  const file = scope === "global" ? GLOBAL_FILE : getLocalFile();
  if (!file) {
    throw new Error("Not in a git repository; cannot save local MCP config");
  }
  const dir = path.dirname(file);
  await fsPromises.mkdir(dir, { recursive: true });

  // Prefer saving in new mcpServers object format if present
  let toSave: any = config;
  if (config.mcpServers) {
    toSave = { mcpServers: config.mcpServers };
  }
  // Otherwise, save legacy format
  await fsPromises.writeFile(file, JSON.stringify(toSave, null, 2), "utf-8");
}

/**
 * List all MCP servers as an array of MCPServer objects.
 */
export async function listMcpServers(
  scope: "global" | "local",
): Promise<Array<MCPServer>> {
  const cfg = await loadConfig(scope);
  return Object.entries(cfg.mcpServers).map(([name, def]) => ({
    name,
    type: "stdio",
    cmd: def.command,
    args: def.args,
    env: def.env,
  }));
}

/**
 * Add a new MCP server to the config.
 */
export async function addMcpServer(
  mcpServer: MCPServer,
  scope: "global" | "local",
): Promise<void> {
  const cfg = await loadConfig(scope);
  if (cfg.mcpServers[mcpServer.name]) {
    throw new Error(
      `MCP Server with name '${mcpServer.name}' already exists in ${scope}`,
    );
  }
  cfg.mcpServers[mcpServer.name] = {
    command: mcpServer.cmd!,
    args: mcpServer.args,
    env: mcpServer.env,
  };
  await saveConfig(scope, cfg);
}

/**
 * Remove an MCP server from the config.
 */
export async function removeMcpServer(
  name: string,
  scope: "global" | "local",
): Promise<void> {
  const cfg = await loadConfig(scope);
  if (!cfg.mcpServers[name]) {
    throw new Error(`MCP server with name '${name}' not found in ${scope}`);
  }
  delete cfg.mcpServers[name];
  await saveConfig(scope, cfg);
}

// Define MCP tool metadata
export interface MCPTool {
  name: string;
  description: string;
  parameters: any; // JSON Schema for tool args
}

// Export serve separately to avoid circular dependencies
export { serveConnection } from "./mcp-serve";
