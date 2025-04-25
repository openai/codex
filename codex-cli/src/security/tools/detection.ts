import { exec } from "child_process";
import { promisify } from "util";
import { createToolsStore } from "../storage/json-store";

const execAsync = promisify(exec);

// Tool interface
export interface Tool {
  id: string;      // Needed for JsonStore
  name: string;
  path: string;
  version: string;
  detected: boolean;
  description: string;
}

// Tool store
const toolStore = createToolsStore<Tool>();

// Common security tools to detect
const COMMON_SECURITY_TOOLS = [
  { name: "nmap", description: "Network scanner" },
  { name: "sqlmap", description: "SQL injection scanner" },
  { name: "nikto", description: "Web server scanner" },
  { name: "gobuster", description: "Directory/file & DNS busting tool" },
  { name: "hydra", description: "Password cracking tool" }
];

/**
 * Detect installed security tools
 */
export async function detectTools(): Promise<Tool[]> {
  const tools: Tool[] = [];
  
  for (const tool of COMMON_SECURITY_TOOLS) {
    let detected = false;
    let path = "";
    let version = "not installed";
    
    try {
      // Check if tool exists using 'which'
      const { stdout } = await execAsync(`which ${tool.name}`);
      path = stdout.trim();
      detected = true;
      
      // Try to get version
      try {
        const { stdout: versionOut } = await execAsync(`${tool.name} --version 2>&1 | head -n 1`);
        // Extract version using a simple regex
        const match = versionOut.match(/\d+\.\d+(\.\d+)?/);
        if (match) {
          version = match[0];
        }
      } catch {
        // Ignore version detection errors
        version = "unknown";
      }
    } catch {
      // Tool not installed
    }
    
    const toolInfo: Tool = {
      id: tool.name,  // Use name as ID
      name: tool.name,
      path,
      version,
      detected,
      description: tool.description
    };
    
    tools.push(toolInfo);
    toolStore.save(toolInfo);
  }
  
  return tools;
}

/**
 * List all detected security tools
 */
export function listTools(): Tool[] {
  return toolStore.getAll();
}

/**
 * Check if a specific tool is installed
 */
export function isToolInstalled(toolName: string): boolean {
  const tool = toolStore.get(toolName);
  return tool?.detected || false;
} 