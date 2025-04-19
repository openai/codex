import { ExecResult } from "../agent/sandbox/interface.js";
import * as ghCli from "./gh-cli.js";

/**
 * Handle GitHub CLI commands
 */
export async function handleGHCommand(
  args: string[]
): Promise<ExecResult> {
  // Check if gh is installed
  const isInstalled = await ghCli.isGhInstalled();
  if (!isInstalled) {
    return {
      stdout: "",
      stderr: "GitHub CLI (gh) is not installed. Please install it first: https://cli.github.com/",
      exitCode: 1,
    };
  }

  // If no arguments provided, run gh with no args (which shows help)
  if (args.length === 0) {
    return ghCli.gh([]);
  }

  // Just pass through all arguments to gh
  return ghCli.gh(args);
}