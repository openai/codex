import type { ExecInput, ExecResult } from "./sandbox/interface.js";
import type { SpawnOptions } from "child_process";
import type { ParseEntry } from "shell-quote";

import { process_patch } from "./apply-patch.js";
import { SandboxType } from "./sandbox/interface.js";
import { execWithSeatbelt } from "./sandbox/macos-seatbelt.js";
import { exec as rawExec } from "./sandbox/raw-exec.js";
import { formatCommandForDisplay } from "../../format-command.js";
import fs from "fs";
import os from "os";
import { parse } from "shell-quote";

const DEFAULT_TIMEOUT_MS = 10_000; // 10 seconds

function requiresShell(cmd: Array<string>): boolean {
  return cmd.some((arg) => {
    const tokens = parse(arg) as Array<ParseEntry>;
    return tokens.some((token) => typeof token === "object" && "op" in token);
  });
}

/**
 * This function should never return a rejected promise: errors should be
 * mapped to a non-zero exit code and the error message should be in stderr.
 */
export function exec(
  {
    cmd,
    workdir,
    timeoutInMillis,
    additionalWritableRoots,
  }: ExecInput & { additionalWritableRoots: ReadonlyArray<string> },
  sandbox: SandboxType,
  abortSignal?: AbortSignal,
): Promise<ExecResult> {
  // This is a temporary measure to understand what are the common base commands
  // until we start persisting and uploading rollouts

  const execForSandbox =
    sandbox === SandboxType.MACOS_SEATBELT ? execWithSeatbelt : rawExec;

  const opts: SpawnOptions = {
    timeout: timeoutInMillis || DEFAULT_TIMEOUT_MS,
    ...(requiresShell(cmd) ? { shell: true } : {}),
    ...(workdir ? { cwd: workdir } : {}),
  };
  // Merge default writable roots with any user-specified ones.
  const writableRoots = [
    process.cwd(),
    os.tmpdir(),
    ...additionalWritableRoots,
  ];
  return execForSandbox(cmd, opts, writableRoots, abortSignal);
}

export function execApplyPatch(patchText: string): ExecResult {
  // This is a temporary measure to understand what are the common base commands
  // until we start persisting and uploading rollouts

  try {
    const result = process_patch(
      patchText,
      (p) => fs.readFileSync(p, "utf8"),
      (p, c) => fs.writeFileSync(p, c, "utf8"),
      (p) => fs.unlinkSync(p),
    );
    return {
      stdout: result,
      stderr: "",
      exitCode: 0,
    };
  } catch (error: unknown) {
    // @ts-expect-error error might not be an object or have a message property.
    const stderr = String(error.message ?? error);
    return {
      stdout: "",
      stderr: stderr,
      exitCode: 1,
    };
  }
}

export function getBaseCmd(cmd: Array<string>): string {
  const formattedCommand = formatCommandForDisplay(cmd);
  return formattedCommand.split(" ")[0] || cmd[0] || "<unknown>";
}
