import type { Result, AnyFlags } from "meow";
import { initSecurityMode } from "./index";
import type { AppConfig } from "../utils/config";

/**
 * Add security mode flags to an existing CLI configuration
 * This function extends the CLI without modifying existing code
 */
export function addSecurityModeFlags(cliInstance: Result<AnyFlags>): void {
  // This is a helper function for documentation, doesn't need implementation
  // It shows what needs to be added to the CLI configuration
}

/**
 * Handle security mode if enabled
 * Returns true if security mode was handled
 */
export async function handleSecurityMode(
  cli: Result<AnyFlags>,
  prompt: string | undefined,
  config: AppConfig
): Promise<boolean> {
  // Add debug logs
  console.log("DEBUG: Checking security mode flag");
  console.log("DEBUG: Security mode flag value:", Boolean(cli.flags['securityMode']));
  console.log("DEBUG: Available CLI flags:", Object.keys(cli.flags));
  
  if (cli.flags['securityMode']) {
    console.log("DEBUG: Security mode detected!");
    
    // If no initial prompt, enter interactive security REPL
    // Any natural-language commands can be entered interactively.
    
    // Determine offensive/predator from flags (supporting '--offensive' and legacy '-offensive')
    const rawArgs = process.argv.slice(2);
    const offensiveFlag = Boolean(cli.flags['offensive']) || rawArgs.includes('-offensive');
    const predatorFlag = Boolean(cli.flags['predator']) || rawArgs.includes('-predator');
    await initSecurityMode(prompt as string, config, {
      target: cli.flags['target'] as string | undefined,
      session: cli.flags['session'] as string | undefined,
      offensive: offensiveFlag,
      predator: predatorFlag,
      playbook: cli.flags['playbook'] as string | undefined,
      dryRun: Boolean(cli.flags['dryRun']),
      generatePlaybook: Boolean(cli.flags['generatePlaybook']),
      mission: cli.flags['mission'] as string | undefined,
      ipcSocket: cli.flags['ipcSocket'] as string | undefined,
      noInstall: Boolean(cli.flags['noInstall']),
      imagePaths: (cli.flags['image'] as string[] | undefined) ?? [],
    });
    
    return true;
  }
  
  return false;
} 