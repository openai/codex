import fs, { promises as fsPromises } from "fs";
import os from "os";
import path from "path";

// Types for MCP server definitions
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
  servers: Array<MCPServer>;
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
    return { servers: [] };
  }
  try {
    const raw = await fsPromises.readFile(file, "utf-8");
    return JSON.parse(raw) as MCPConfig;
  } catch {
    return { servers: [] };
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
  await fsPromises.writeFile(file, JSON.stringify(config, null, 2), "utf-8");
}

// Public API
export async function listServers(
  scope: "global" | "local",
): Promise<Array<MCPServer>> {
  const cfg = await loadConfig(scope);
  return cfg.servers;
}

export async function addServer(
  server: MCPServer,
  scope: "global" | "local",
): Promise<void> {
  const cfg = await loadConfig(scope);
  if (cfg.servers.find((s) => s.name === server.name)) {
    throw new Error(
      `Server with name '${server.name}' already exists in ${scope}`,
    );
  }
  cfg.servers.push(server);
  await saveConfig(scope, cfg);
}

export async function removeServer(
  name: string,
  scope: "global" | "local",
): Promise<void> {
  const cfg = await loadConfig(scope);
  const idx = cfg.servers.findIndex((s) => s.name === name);
  if (idx < 0) {
    throw new Error(`Server with name '${name}' not found in ${scope}`);
  }
  cfg.servers.splice(idx, 1);
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