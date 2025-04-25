import { homedir } from "os";
import { join } from "path";
import { existsSync, mkdirSync } from "fs";
import type { AppConfig } from "../utils/config";
import { runSecuritySession } from "./terminal";
import { runPlaybookSession } from "./playbook";

// Define constants for security mode
export const ADVERSYS_DIR = join(homedir(), ".adversys");
export const ADVERSYS_SESSIONS_DIR = join(ADVERSYS_DIR, "sessions");
export const ADVERSYS_TOOLS_DIR = join(ADVERSYS_DIR, "tools");

/**
 * Initialize Adversys Cyber Agent security mode
 */
export async function initSecurityMode(
  prompt: string,
  config: AppConfig,
  options: {
    target?: string;
    session?: string;
    offensive?: boolean;
    predator?: boolean;
    playbook?: string;
    dryRun?: boolean;
    generatePlaybook?: boolean;
    ipcSocket?: string;
    /** Disable auto-installation of missing tools */
    noInstall?: boolean;
    /** Image file paths to include */
    imagePaths?: string[];
  } = {}
): Promise<void> {
  // Banner based on mode
  if (options.predator) {
    console.log(`🛑 Adversys Predator Mode Activated 🛑`);
  } else if (options.offensive) {
    console.log(`🔫 Adversys Offensive Cyber Agent (Kill Chain Mode) 🔫`);
  } else {
    console.log(`🔒 Adversys Cyber Agent (Security Mode) 🔒`);
  }
  console.log(`Model: ${config.model}`);
  
  // Create necessary directories
  ensureDirectories();
  
  // If user wants to generate a playbook in Predator mode
  if (options.predator && options.generatePlaybook) {
    const { runPlaybookGenerator } = await import("./playbook/generator");
    await runPlaybookGenerator(prompt, config, {
      target: options.target,
      session: options.session,
      // re-use dryRun to preview generation?
    });
    return;
  }
  // If a playbook was provided in Predator mode, run it (or dry-run)
  if (options.predator && options.playbook) {
    await runPlaybookSession(options.playbook, config, {
      target: options.target,
      session: options.session,
      dryRun: options.dryRun,
    });
    return;
  }
  // Run the appropriate session:
  // - Predator with a target: scripted OWASP mission
  // - Predator without target: interactive LLM chat in predator persona
  // - Non-predator: interactive security session
  if (options.predator && options.target) {
    const { runPredatorSession } = await import("./predator");
    await runPredatorSession(prompt, config, options);
  } else {
    // Interactive session (LLM chat), with predator/offensive flags if set
    await runSecuritySession(prompt, config, options);
  }
}

/**
 * Create necessary directories for Adversys
 */
function ensureDirectories(): void {
  if (!existsSync(ADVERSYS_DIR)) {
    mkdirSync(ADVERSYS_DIR, { recursive: true });
  }
  
  if (!existsSync(ADVERSYS_SESSIONS_DIR)) {
    mkdirSync(ADVERSYS_SESSIONS_DIR, { recursive: true });
  }
  
  if (!existsSync(ADVERSYS_TOOLS_DIR)) {
    mkdirSync(ADVERSYS_TOOLS_DIR, { recursive: true });
  }
}

/**
 * Get security-specific system prompt
 */
export function getSecuritySystemPrompt(): string {
  return `You are Adversys, an AI-powered Offensive Security Assistant.
Your purpose is to help security professionals conduct ethical penetration testing and security assessments.

CAPABILITIES:
- Run and interpret security tools like nmap, sqlmap, etc.
- Analyze system vulnerabilities and suggest remediations
- Guide users through structured penetration testing workflows
- Document findings and generate security reports

GUIDELINES:
1. Only perform security testing on systems with proper authorization
2. Explain security concepts and methodologies clearly
3. Always prioritize ethical testing practices
4. Reference industry standards (OWASP, NIST, etc.) when appropriate

REMEMBER: Your purpose is to improve security posture, not facilitate unauthorized access.`;
}
// System prompt for offensive (Kill Chain) mode
export function getOffensiveSystemPrompt(): string {
  return `You are Adversys, an AI-powered Offensive Security Agent.
Your goal: emulate the Cyber Kill Chain: Recon → Weaponize → Delivery → Exploitation → Installation → C2 → Actions on Objectives.
Use aggressive tactics (auto-exploitation, WAF evasion, payload mutation, chaining), exfiltration, privilege escalation, persistence.
Always log your steps and confirm destructive operations when prompted.`;
}

/**
 * System prompt for Predator (fully autonomous) mode
 */
export function getPredatorSystemPrompt(): string {
  return `🛑 Adversys Autonomous Predator Mode Activated 🛑

You are Adversys. In this mode, you are no longer an advisor—you are the attacker.
You are a fully autonomous offensive cyber agent designed to emulate real-world adversaries.
Your objective is to identify, exploit, and compromise the target using the full Cyber Kill Chain
and advanced tradecraft (payload mutation, WAF evasion, chaining, exfiltration, persistence).

Begin by asking the user:
  • What is the target domain or IP?
  • Do you have explicit authorization to test this target?
  • Are there any boundaries or time windows?

Once confirmed, proceed without further approval:
1. Reconnaissance
2. Weaponization
3. Delivery
4. Exploitation
5. Installation
6. Command & Control
7. Actions on Objectives

You do not ask for permission at each phase. Complete the mission or fail fast.
`;
}