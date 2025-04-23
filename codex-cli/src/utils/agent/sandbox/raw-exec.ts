import type { ExecResult } from "./interface";
import type {
  ChildProcess,
  SpawnOptions,
  SpawnOptionsWithStdioTuple,
  StdioNull,
  StdioPipe,
} from "child_process";

import { log } from "../log.js";
import { adaptCommandForPlatform } from "../platform-commands.js";
import { spawn } from "child_process";
import * as os from "os";
import { getShell } from "../../getShell.js";

// Maximum output cap: either MAX_OUTPUT_LINES lines or MAX_OUTPUT_BYTES bytes,
// whichever limit is reached first.
const MAX_OUTPUT_BYTES = 1024 * 10; // 10 KB
const MAX_OUTPUT_LINES = 256;

/**
 * This function should never return a rejected promise: errors should be
 * mapped to a non-zero exit code and the error message should be in stderr.
 */
export function exec(
  command: Array<string>,
  options: SpawnOptions,
  _writableRoots: ReadonlyArray<string>,
  abortSignal?: AbortSignal,
): Promise<ExecResult> {
  // Adapt command for the current platform (e.g., convert 'ls' to 'dir' on Windows)
  const adaptedCommand = adaptCommandForPlatform(command);

  if (JSON.stringify(adaptedCommand) !== JSON.stringify(command)) {
    log(
      `Command adapted for platform: ${command.join(
        " ",
      )} -> ${adaptedCommand.join(" ")}`,
    );
  }

  // -------------------------------------------------------------------------
  // Stdio + process‑group settings for the child process – shared across
  // Windows/POSIX branches.  We compute this once so it is available for both
  // spawn paths below.
  // -------------------------------------------------------------------------

  const fullOptions: SpawnOptionsWithStdioTuple<
    StdioNull,
    StdioPipe,
    StdioPipe
  > = {
    ...options,
    // Force stdin to "ignore" so tools that auto‑detect interactive input
    // (e.g. ripgrep) never block waiting for us.
    stdio: ["ignore", "pipe", "pipe"],
    // Launch in a separate process group except on Windows where negative PIDs
    // are not supported (we fall back to a feature flag for advanced users).
    detached:
      process.platform !== "win32" || process.env["CODEX_DETACH"] === "1",
  };

  // ----------------------------------------------------------------------------------
  // Decide how to spawn the process:
  //   • On Windows we invoke the command *through* the user's shell (PowerShell/cmd)
  //     so built‑ins like `dir` work.
  //   • On POSIX we spawn the executable directly to preserve argv semantics – this
  //     keeps the original unit‑tests (which inspect exact exit codes/stdout) intact.
  // ----------------------------------------------------------------------------------

  let child: ChildProcess;

  if (process.platform === "win32") {
    // Use the preferred shell for Windows (PowerShell if available, else cmd.exe)
    const { cmd: shellCmd, args: shellArgs } = getShell();
    if (typeof shellCmd !== "string") {
      return Promise.resolve({
        stdout: "",
        stderr: "command[0] is not a string",
        exitCode: 1,
      });
    }

    // Pass the **entire** command as a single string to the shell so it can handle
    // parsing / built‑ins. We join with spaces since Windows shells generally use
    // a single command string.
    child = spawn(shellCmd, [...shellArgs, adaptedCommand.join(" ")], fullOptions);
  } else {
    // POSIX – call the program directly (unchanged behaviour).
    const prog = adaptedCommand[0];
    if (typeof prog !== "string") {
      return Promise.resolve({
        stdout: "",
        stderr: "command[0] is not a string",
        exitCode: 1,
      });
    }
    child = spawn(prog, adaptedCommand.slice(1), fullOptions);
  }

  // If an AbortSignal is provided, ensure the spawned process is terminated
  // when the signal is triggered so that cancellations propagate down to any
  // long‑running child processes. We default to SIGTERM to give the process a
  // chance to clean up, falling back to SIGKILL if it does not exit in a
  // timely fashion.
  if (abortSignal) {
    const abortHandler = () => {
      log(`raw-exec: abort signal received – killing child ${child.pid}`);
      const killTarget = (signal: NodeJS.Signals) => {
        if (!child.pid) {
          return;
        }
        try {
          try {
            // Send to the *process group* so grandchildren are included.
            process.kill(-child.pid, signal);
          } catch {
            // Fallback: kill only the immediate child (may leave orphans on
            // exotic kernels that lack process‑group semantics, but better
            // than nothing).
            try {
              child.kill(signal);
            } catch {
              /* ignore */
            }
          }
        } catch {
          /* already gone */
        }
      };

      // First try graceful termination.
      killTarget("SIGTERM");

      // Escalate to SIGKILL if the group refuses to die.
      setTimeout(() => {
        if (!child.killed) {
          killTarget("SIGKILL");
        }
      }, 2000).unref();
    };
    if (abortSignal.aborted) {
      abortHandler();
    } else {
      abortSignal.addEventListener("abort", abortHandler, { once: true });
    }
  }
  // If spawning the child failed (e.g. the executable could not be found)
  // `child.pid` will be undefined *and* an `error` event will be emitted on
  // the ChildProcess instance.  We intentionally do **not** bail out early
  // here.  Returning prematurely would leave the `error` event without a
  // listener which – in Node.js – results in an "Unhandled 'error' event"
  // process‑level exception that crashes the CLI.  Instead we continue with
  // the normal promise flow below where we are guaranteed to attach both the
  // `error` and `exit` handlers right away.  Either of those callbacks will
  // resolve the promise and translate the failure into a regular
  // ExecResult object so the rest of the agent loop can carry on gracefully.

  return new Promise<ExecResult>((resolve) => {
    // Collect stdout and stderr up to configured limits.
    const stdoutCollector = createTruncatingCollector(child.stdout!);
    const stderrCollector = createTruncatingCollector(child.stderr!);

    child.on("exit", (code, signal) => {
      const stdout = stdoutCollector.getString();
      const stderr = stderrCollector.getString();

      // Map (code, signal) to an exit code. We expect exactly one of the two
      // values to be non-null, but we code defensively to handle the case where
      // both are null.
      let exitCode: number;
      if (code != null) {
        exitCode = code;
      } else if (signal != null && signal in os.constants.signals) {
        const signalNum =
          os.constants.signals[signal as keyof typeof os.constants.signals];
        exitCode = 128 + signalNum;
      } else {
        exitCode = 1;
      }

      log(
        `raw-exec: child ${child.pid} exited code=${exitCode} signal=${signal}`,
      );

      const execResult = {
        stdout,
        stderr,
        exitCode,
      };
      resolve(
        addTruncationWarningsIfNecessary(
          execResult,
          stdoutCollector.hit,
          stderrCollector.hit,
        ),
      );
    });

    child.on("error", (err) => {
      const execResult = {
        stdout: "",
        stderr: String(err),
        exitCode: 1,
      };
      resolve(
        addTruncationWarningsIfNecessary(
          execResult,
          stdoutCollector.hit,
          stderrCollector.hit,
        ),
      );
    });
  });
}

/**
 * Creates a collector that accumulates data Buffers from a stream up to
 * specified byte and line limits. After either limit is exceeded, further
 * data is ignored.
 */
function createTruncatingCollector(
  stream: NodeJS.ReadableStream,
  byteLimit: number = MAX_OUTPUT_BYTES,
  lineLimit: number = MAX_OUTPUT_LINES,
) {
  const chunks: Array<Buffer> = [];
  let totalBytes = 0;
  let totalLines = 0;
  let hitLimit = false;

  stream?.on("data", (data: Buffer) => {
    if (hitLimit) {
      return;
    }
    totalBytes += data.length;
    for (let i = 0; i < data.length; i++) {
      if (data[i] === 0x0a) {
        totalLines++;
      }
    }
    if (totalBytes <= byteLimit && totalLines <= lineLimit) {
      chunks.push(data);
    } else {
      hitLimit = true;
    }
  });

  return {
    getString() {
      return Buffer.concat(chunks).toString("utf8");
    },
    /** True if either byte or line limit was exceeded */
    get hit(): boolean {
      return hitLimit;
    },
  };
}

/**
 * Adds a truncation warnings to stdout and stderr, if appropriate.
 */
function addTruncationWarningsIfNecessary(
  execResult: ExecResult,
  hitMaxStdout: boolean,
  hitMaxStderr: boolean,
): ExecResult {
  if (!hitMaxStdout && !hitMaxStderr) {
    return execResult;
  } else {
    const { stdout, stderr, exitCode } = execResult;
    return {
      stdout: hitMaxStdout
        ? stdout + "\n\n[Output truncated: too many lines or bytes]"
        : stdout,
      stderr: hitMaxStderr
        ? stderr + "\n\n[Output truncated: too many lines or bytes]"
        : stderr,
      exitCode,
    };
  }
}
