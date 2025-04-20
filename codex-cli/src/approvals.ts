import type { ParseEntry, ControlOperator } from "shell-quote";

import {
  identify_files_added,
  identify_files_needed,
} from "./utils/agent/apply-patch";
import { loadConfig } from "./utils/config";
import * as path from "path";
import { parse } from "shell-quote";

export type SafetyAssessment = {
  /**
   * If set, this approval is for an apply_patch call and these are the
   * arguments.
   */
  applyPatch?: ApplyPatchCommand;
} & (
  | {
      type: "auto-approve";
      /**
       * This must be true if the command is not on the "known safe" list, but
       * was auto-approved due to `full-auto` mode.
       */
      runInSandbox: boolean;
      reason: string;
      group: string;
    }
  | {
      type: "ask-user";
    }
  /**
   * Reserved for a case where we are certain the command is unsafe and should
   * not be presented as an option to the user.
   */
  | {
      type: "reject";
      reason: string;
    }
);

// TODO: This should also contain the paths that will be affected.
export type ApplyPatchCommand = {
  patch: string;
};

export type ApprovalPolicy =
  /**
   * Under this policy, only "known safe" commands as defined by
   * `isSafeCommand()` that only read files will be auto-approved.
   */
  | "suggest"

  /**
   * In addition to commands that are auto-approved according to the rules for
   * "suggest", commands that write files within the user's approved list of
   * writable paths will also be auto-approved.
   */
  | "auto-edit"

  /**
   * All commands are auto-approved, but are expected to be run in a sandbox
   * where network access is disabled and writes are limited to a specific set
   * of paths.
   */
  | "full-auto";

/**
 * Tries to assess whether a command is safe to run, though may defer to the
 * user for approval.
 *
 * Note `env` must be the same `env` that will be used to spawn the process.
 */
export function canAutoApprove(
  command: ReadonlyArray<string>,
  policy: ApprovalPolicy,
  writableRoots: ReadonlyArray<string>,
  env: NodeJS.ProcessEnv = process.env,
): SafetyAssessment {
  if (command[0] === "apply_patch") {
    return command.length === 2 && typeof command[1] === "string"
      ? canAutoApproveApplyPatch(command[1], writableRoots, policy)
      : {
          type: "reject",
          reason: "Invalid apply_patch command",
        };
  }

  const isSafe = isSafeCommand(command);
  if (isSafe != null) {
    const { reason, group } = isSafe;
    return {
      type: "auto-approve",
      reason,
      group,
      runInSandbox: false,
    };
  }

  if (
    command[0] === "bash" &&
    command[1] === "-lc" &&
    typeof command[2] === "string" &&
    command.length === 3
  ) {
    const applyPatchArg = tryParseApplyPatch(command[2]);
    if (applyPatchArg != null) {
      return canAutoApproveApplyPatch(applyPatchArg, writableRoots, policy);
    }

    let bashCmd;
    try {
      bashCmd = parse(command[2], env);
    } catch (e) {
      // In practice, there seem to be syntactically valid shell commands that
      // shell-quote cannot parse, so we should not reject, but ask the user.
      switch (policy) {
        case "full-auto":
          // In full-auto, we still run the command automatically, but must
          // restrict it to the sandbox.
          return {
            type: "auto-approve",
            reason: "Full auto mode",
            group: "Running commands",
            runInSandbox: true,
          };
        case "suggest":
        case "auto-edit":
          // In all other modes, since we cannot reason about the command, we
          // should ask the user.
          return {
            type: "ask-user",
          };
      }
    }

    // bashCmd could be a mix of strings and operators, e.g.:
    //   "ls || (true && pwd)" => [ 'ls', { op: '||' }, '(', 'true', { op: '&&' }, 'pwd', ')' ]
    // We try to ensure that *every* command segment is deemed safe and that
    // all operators belong to an allow‑list. If so, the entire expression is
    // considered auto‑approvable.

    const shellSafe = isEntireShellExpressionSafe(bashCmd);
    if (shellSafe != null) {
      const { reason, group } = shellSafe;
      return {
        type: "auto-approve",
        reason,
        group,
        runInSandbox: false,
      };
    }
  }

  return policy === "full-auto"
    ? {
        type: "auto-approve",
        reason: "Full auto mode",
        group: "Running commands",
        runInSandbox: true,
      }
    : { type: "ask-user" };
}

function canAutoApproveApplyPatch(
  applyPatchArg: string,
  writableRoots: ReadonlyArray<string>,
  policy: ApprovalPolicy,
): SafetyAssessment {
  switch (policy) {
    case "full-auto":
      // Continue to see if this can be auto-approved.
      break;
    case "suggest":
      return {
        type: "ask-user",
        applyPatch: { patch: applyPatchArg },
      };
    case "auto-edit":
      // Continue to see if this can be auto-approved.
      break;
  }

  if (isWritePatchConstrainedToWritablePaths(applyPatchArg, writableRoots)) {
    return {
      type: "auto-approve",
      reason: "apply_patch command is constrained to writable paths",
      group: "Editing",
      runInSandbox: false,
      applyPatch: { patch: applyPatchArg },
    };
  }

  return policy === "full-auto"
    ? {
        type: "auto-approve",
        reason: "Full auto mode",
        group: "Editing",
        runInSandbox: true,
        applyPatch: { patch: applyPatchArg },
      }
    : {
        type: "ask-user",
        applyPatch: { patch: applyPatchArg },
      };
}

/**
 * All items in `writablePaths` must be absolute paths.
 */
function isWritePatchConstrainedToWritablePaths(
  applyPatchArg: string,
  writableRoots: ReadonlyArray<string>,
): boolean {
  // `identify_files_needed()` returns a list of files that will be modified or
  // deleted by the patch, so all of them should already exist on disk. These
  // candidate paths could be further canonicalized via fs.realpath(), though
  // that does seem necessary and may even cause false negatives (assuming we
  // allow writes in other directories that are symlinked from a writable path)
  //
  // By comparison, `identify_files_added()` returns a list of files that will
  // be added by the patch, so they should NOT exist on disk yet and therefore
  // using one with fs.realpath() should return an error.
  return (
    allPathsConstrainedTowritablePaths(
      identify_files_needed(applyPatchArg),
      writableRoots,
    ) &&
    allPathsConstrainedTowritablePaths(
      identify_files_added(applyPatchArg),
      writableRoots,
    )
  );
}

function allPathsConstrainedTowritablePaths(
  candidatePaths: ReadonlyArray<string>,
  writableRoots: ReadonlyArray<string>,
): boolean {
  return candidatePaths.every((candidatePath) =>
    isPathConstrainedTowritablePaths(candidatePath, writableRoots),
  );
}

/** If candidatePath is relative, it will be resolved against cwd. */
function isPathConstrainedTowritablePaths(
  candidatePath: string,
  writableRoots: ReadonlyArray<string>,
): boolean {
  const candidateAbsolutePath = path.resolve(candidatePath);
  return writableRoots.some((writablePath) =>
    pathContains(writablePath, candidateAbsolutePath),
  );
}

/** Both `parent` and `child` must be absolute paths. */
function pathContains(parent: string, child: string): boolean {
  const relative = path.relative(parent, child);
  return (
    // relative path doesn't go outside parent
    !!relative && !relative.startsWith("..") && !path.isAbsolute(relative)
  );
}

/**
 * `bashArg` might be something like "apply_patch << 'EOF' *** Begin...".
 * If this function returns a string, then it is the content the arg to
 * apply_patch with the heredoc removed.
 */
function tryParseApplyPatch(bashArg: string): string | null {
  const prefix = "apply_patch";
  if (!bashArg.startsWith(prefix)) {
    return null;
  }

  const heredoc = bashArg.slice(prefix.length);
  const heredocMatch = heredoc.match(
    /^\s*<<\s*['"]?(\w+)['"]?\n([\s\S]*?)\n\1/,
  );
  if (heredocMatch != null && typeof heredocMatch[2] === "string") {
    return heredocMatch[2].trim();
  } else {
    return heredoc.trim();
  }
}

export type SafeCommandReason = {
  reason: string;
  group: string;
};

/**
 * If this is a "known safe" command, returns the (reason, group); otherwise,
 * returns null.
 */
export function isSafeCommand(
  command: ReadonlyArray<string>,
): SafeCommandReason | null {
  const [cmd0, cmd1, cmd2, cmd3] = command;

  const config = loadConfig();
  if (config.safeCommands && Array.isArray(config.safeCommands)) {
    for (const safe of config.safeCommands) {
      // safe: "npm test" → ["npm", "test"]
      const safeArr = typeof safe === "string" ? safe.trim().split(/\s+/) : [];
      if (
        safeArr.length > 0 &&
        safeArr.length <= command.length &&
        safeArr.every((v, i) => v === command[i])
      ) {
        return {
          reason: "User-defined safe command",
          group: "User config",
        };
      }
    }
  }

  switch (cmd0) {
    case "cd":
      return {
        reason: "Change directory",
        group: "Navigating",
      };
    case "ls":
      return {
        reason: "List directory",
        group: "Searching",
      };
    case "pwd":
      return {
        reason: "Print working directory",
        group: "Navigating",
      };
    case "true":
      return {
        reason: "No‑op (true)",
        group: "Utility",
      };
    case "echo":
      return { reason: "Echo string", group: "Printing" };
    case "cat":
      return {
        reason: "View file contents",
        group: "Reading files",
      };
    case "rg":
      return {
        reason: "Ripgrep search",
        group: "Searching",
      };
    case "find":
      return {
        reason: "Find files or directories",
        group: "Searching",
      };
    case "grep":
      return {
        reason: "Text search (grep)",
        group: "Searching",
      };
    case "head":
      return {
        reason: "Show file head",
        group: "Reading files",
      };
    case "tail":
      return {
        reason: "Show file tail",
        group: "Reading files",
      };
    case "wc":
      return {
        reason: "Word count",
        group: "Reading files",
      };
    case "which":
      return {
        reason: "Locate command",
        group: "Searching",
      };
    case "git":
      switch (cmd1) {
        case "status":
          return {
            reason: "Git status",
            group: "Versioning",
          };
        case "branch":
          return {
            reason: "List Git branches",
            group: "Versioning",
          };
        case "log":
          return {
            reason: "Git log",
            group: "Using git",
          };
        case "diff":
          return {
            reason: "Git diff",
            group: "Using git",
          };
        case "show":
          return {
            reason: "Git show",
            group: "Using git",
          };
        default:
          return null;
      }
    case "cargo":
      if (cmd1 === "check") {
        return {
          reason: "Cargo check",
          group: "Running command",
        };
      }
      break;
    case "sed":
      if (
        cmd1 === "-n" &&
        isValidSedNArg(cmd2) &&
        typeof cmd3 === "string" &&
        command.length === 4
      ) {
        return {
          reason: "Sed print subset",
          group: "Reading files",
        };
      }
      break;
    default:
      return null;
  }

  return null;
}

function isValidSedNArg(arg: string | undefined): boolean {
  return arg != null && /^(\d+,)?\d+p$/.test(arg);
}

// ---------------- Helper utilities for complex shell expressions -----------------

// A conservative allow‑list of bash operators that do not, on their own, cause
// side effects. Redirections (>, >>, <, etc.) and command substitution `$()`
// are intentionally excluded. Parentheses used for grouping are treated as
// strings by `shell‑quote`, so we do not add them here. Reference:
// https://github.com/substack/node-shell-quote#parsecmd-opts
const SAFE_SHELL_OPERATORS: ReadonlySet<string> = new Set([
  "&&", // logical AND
  "||", // logical OR
  "|", // pipe
  ";", // command separator
]);

/**
 * Determines whether a parsed shell expression consists solely of safe
 * commands (as per `isSafeCommand`) combined using only operators in
 * `SAFE_SHELL_OPERATORS`.
 *
 * If entirely safe, returns the reason/group from the *first* command
 * segment so callers can surface a meaningful description. Otherwise returns
 * null.
 */
function isEntireShellExpressionSafe(
  parts: ReadonlyArray<ParseEntry>,
): SafeCommandReason | null {
  if (parts.length === 0) {
    return null;
  }

  try {
    // Collect command segments delimited by operators. `shell‑quote` represents
    // subshell grouping parentheses as literal strings "(" and ")"; treat them
    // as unsafe to keep the logic simple (since subshells could introduce
    // unexpected scope changes).

    let currentSegment: Array<string> = [];
    let firstReason: SafeCommandReason | null = null;

    const flushSegment = (): boolean => {
      if (currentSegment.length === 0) {
        return true; // nothing to validate (possible leading operator)
      }
      const assessment = isSafeCommand(currentSegment);
      if (assessment == null) {
        return false;
      }
      if (firstReason == null) {
        firstReason = assessment;
      }
      currentSegment = [];
      return true;
    };

    for (const part of parts) {
      if (typeof part === "string") {
        // If this string looks like an open/close parenthesis or brace, treat as
        // unsafe to avoid parsing complexity.
        if (part === "(" || part === ")" || part === "{" || part === "}") {
          return null;
        }
        currentSegment.push(part);
      } else if (isParseEntryWithOp(part)) {
        // Validate the segment accumulated so far.
        if (!flushSegment()) {
          return null;
        }

        // Validate the operator itself.
        if (!SAFE_SHELL_OPERATORS.has(part.op)) {
          return null;
        }
      } else {
        // Unknown token type
        return null;
      }
    }

    // Validate any trailing command segment.
    if (!flushSegment()) {
      return null;
    }

    return firstReason;
  } catch (_err) {
    // If there's any kind of failure, just bail out and return null.
    return null;
  }
}

// Runtime type guard that narrows a `ParseEntry` to the variants that
// carry an `op` field. Using a dedicated function avoids the need for
// inline type assertions and makes the narrowing reusable and explicit.
function isParseEntryWithOp(
  entry: ParseEntry,
): entry is { op: ControlOperator } | { op: "glob"; pattern: string } {
  return (
    typeof entry === "object" &&
    entry != null &&
    // Using the safe `in` operator keeps the check property‑safe even when
    // `entry` is a `string`.
    "op" in entry &&
    typeof (entry as { op?: unknown }).op === "string"
  );
}
