import { spawn } from "child_process";
import type { AppConfig } from "../utils/config";
import { createSession, getSessionByName, recordCommand } from "./storage/sessions";
import { detectTools, listTools } from "./tools/detection";
import OpenAI from "openai";
import { getSecuritySystemPrompt } from "./index";
import readline from "readline";

// Define the interface for command output
interface CommandOutput {
  stdout: string;
  stderr: string;
  exitCode: number;
}

/**
 * Run a security session with minimal dependencies
 */
import { runInteractiveSession } from './interactive';

export async function runSecuritySession(
  prompt: string | undefined,
  config: AppConfig,
  options: { target?: string; session?: string; offensive?: boolean; predator?: boolean; ipcSocket?: string; noInstall?: boolean; imagePaths?: string[] } = {}
): Promise<void> {
  // Delegate to full interactive session (PTY + streaming chat)
  await runInteractiveSession(prompt, config, options);
}

/**
 * Run a command and return its output
 */
async function runSecurityCommand(command: string): Promise<CommandOutput> {
  return new Promise((resolve, reject) => {
    let stdout = "";
    let stderr = "";
    
    const proc = spawn("bash", ["-c", command], { shell: true });
    
    proc.stdout.on("data", (data) => {
      const chunk = data.toString();
      stdout += chunk;
      process.stdout.write(chunk);
    });
    
    proc.stderr.on("data", (data) => {
      const chunk = data.toString();
      stderr += chunk;
      process.stderr.write(chunk);
    });
    
    proc.on("close", (code) => {
      resolve({
        stdout,
        stderr,
        exitCode: code || 0
      });
    });
    
    proc.on("error", (error) => {
      reject(error);
    });
  });
}

/**
 * Display help information
 */
function displayHelp(): void {
  console.log(`
Adversys Cyber Agent Commands:

  scan <target>              Simple wrapper for nmap scan
  tools                      List detected security tools
  sessions                   List active sessions
  help                       Display this help message
  exit                       Exit the security mode
  
You can also run any standard command or security tool directly.
`);
} 