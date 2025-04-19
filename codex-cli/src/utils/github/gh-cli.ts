import type { ExecResult } from "../agent/sandbox/interface.js";
import { exec } from "../agent/exec.js";
import { SandboxType } from "../agent/sandbox/interface.js";

/**
 * GitHub CLI (gh) utility functions
 */

/**
 * Check if GitHub CLI is installed
 */
export async function isGhInstalled(): Promise<boolean> {
  try {
    const result = await exec(
      { 
        cmd: ["gh", "--version"], 
        workdir: undefined, 
        timeoutInMillis: 5_000, 
        additionalWritableRoots: [] 
      },
      SandboxType.NONE
    );
    return result.exitCode === 0;
  } catch (error) {
    return false;
  }
}

/**
 * Execute GitHub CLI commands
 */
export async function gh(
  args: string[],
  opts: { workdir?: string; timeoutInMillis?: number } = {},
): Promise<ExecResult> {
  const { workdir, timeoutInMillis } = opts;
  return exec(
    {
      cmd: ["gh", ...args],
      workdir,
      timeoutInMillis,
      additionalWritableRoots: [],
    },
    SandboxType.NONE
  );
}

/**
 * Create a GitHub issue
 */
export async function createIssue(
  title: string,
  body: string,
  repo?: string
): Promise<ExecResult> {
  const args = ["issue", "create", "--title", title, "--body", body];
  
  if (repo) {
    args.push("--repo", repo);
  }
  
  return gh(args);
}

/**
 * List GitHub issues
 */
export async function listIssues(
  state: "open" | "closed" | "all" = "open",
  limit: number = 10,
  repo?: string
): Promise<ExecResult> {
  const args = ["issue", "list", "--state", state, "--limit", String(limit)];
  
  if (repo) {
    args.push("--repo", repo);
  }
  
  return gh(args);
}

/**
 * Create a GitHub pull request
 */
export async function createPR(
  title: string,
  body: string,
  baseBranch?: string,
  repo?: string
): Promise<ExecResult> {
  const args = ["pr", "create", "--title", title, "--body", body];
  
  if (baseBranch) {
    args.push("--base", baseBranch);
  }
  
  if (repo) {
    args.push("--repo", repo);
  }
  
  return gh(args);
}

/**
 * List GitHub pull requests
 */
export async function listPRs(
  state: "open" | "closed" | "merged" | "all" = "open",
  limit: number = 10,
  repo?: string
): Promise<ExecResult> {
  const args = ["pr", "list", "--state", state, "--limit", String(limit)];
  
  if (repo) {
    args.push("--repo", repo);
  }
  
  return gh(args);
}

/**
 * View a GitHub workflow
 */
export async function viewWorkflow(
  workflowId: string,
  repo?: string
): Promise<ExecResult> {
  const args = ["workflow", "view", workflowId];
  
  if (repo) {
    args.push("--repo", repo);
  }
  
  return gh(args);
}

/**
 * Run a GitHub workflow
 */
export async function runWorkflow(
  workflowId: string,
  ref?: string,
  repo?: string
): Promise<ExecResult> {
  const args = ["workflow", "run", workflowId];
  
  if (ref) {
    args.push("--ref", ref);
  }
  
  if (repo) {
    args.push("--repo", repo);
  }
  
  return gh(args);
}