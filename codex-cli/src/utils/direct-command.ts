import type { CommandConfirmation } from "./agent/agent-loop.js";
import type { ApplyPatchCommand } from "../approvals.js";
import type { AppConfig } from "./config.js";
import type { ExecResult } from "./agent/sandbox/interface.js";
import { handleExecCommand } from "./agent/handle-exec-command.js";
import { ReviewDecision } from "./agent/review.js";
import { parse } from "shell-quote";

/**
 * Handler for direct command execution with prefixed commands (! or $)
 * ! - Execute command without adding to context
 * $ - Execute command and add output to context
 */
export async function handleDirectCommand(
  rawCommand: string,
  config: AppConfig,
  getCommandConfirmation: (
    command: Array<string>,
    applyPatch: ApplyPatchCommand | undefined,
  ) => Promise<CommandConfirmation>
): Promise<DirectCommandResult> {
  // Strip the prefix (! or $)
  const prefix = rawCommand.charAt(0);
  const command = rawCommand.slice(1).trim();
  
  // Split into argument array using shell-quote for proper parsing
  const args = parseCommandIntoArgs(command);
  
  // Use either auto-approval or standard approval flow based on configuration
  const approval = getApproval(config, getCommandConfirmation);
  
  // Use the existing execution path
  const result = await handleExecCommand(
    { cmd: args },
    config,
    'auto-edit', // Use existing approval mode
    [], // No additional writable roots
    approval
  );
  
  // Determine if we should add this command output to context
  // $ prefix always adds to context
  // ! prefix never adds to context
  // The config setting is only used for $ prefix as an additional gate
  const addToContext = prefix === '$' && 
    (config.directCommands?.addToContext !== false);
  
  return {
    outputText: result.outputText,
    metadata: result.metadata,
    prefix,
    originalCommand: rawCommand,
    addToContext
  };
}

/**
 * Determines whether to use auto-approval or standard approval workflow
 * based on user configuration
 */
function getApproval(
  config: AppConfig, 
  getCommandConfirmation: (
    command: Array<string>,
    applyPatch: ApplyPatchCommand | undefined,
  ) => Promise<CommandConfirmation>
): (
  command: Array<string>,
  applyPatch: ApplyPatchCommand | undefined,
) => Promise<CommandConfirmation> {
  // Check if user has explicitly opted in to auto-approval for direct commands
  const autoApprove = config.directCommands?.autoApprove === true;
  
  if (autoApprove) {
    // Auto-approve if user has explicitly enabled this in config
    return async (command: Array<string>, applyPatch?: ApplyPatchCommand): 
      Promise<CommandConfirmation> => {
      return {
        review: ReviewDecision.YES,
        command
      };
    };
  } else {
    // Otherwise use the standard approval flow
    // This will prompt the user for confirmation
    return getCommandConfirmation;
  }
}

/**
 * Utility to parse command string into args array
 * using shell-quote to handle quotes, escapes, etc.
 */
function parseCommandIntoArgs(command: string): string[] {
  try {
    const parsed = parse(command);
    // Convert all items to strings and filter out any non-string elements
    return parsed.filter(item => typeof item === 'string') as string[];
  } catch (error) {
    // If parsing fails, fall back to a simple split by space
    return command.split(/\s+/).filter(Boolean);
  }
}

/**
 * Extended ExecResult type with prefix and original command information
 */
export interface DirectCommandResult extends ExecResult {
  prefix: string;
  originalCommand: string;
  addToContext: boolean;
}

/**
 * Check if a result is from a direct command
 */
export function isDirectCommandResult(result: any): result is DirectCommandResult {
  return result && 
    typeof result.prefix === 'string' && 
    typeof result.originalCommand === 'string' &&
    typeof result.addToContext === 'boolean';
}