import type { ExecResult } from "./interface";
import type { AppConfig } from "../../config";
import type {
  ChildProcess,
  SpawnOptions,
  SpawnOptionsWithStdioTuple,
  StdioNull,
  StdioPipe,
} from "child_process";

import { log } from "../../logger/log.js";
import { adaptCommandForPlatform } from "../platform-commands.js";
import { createTruncatingCollector } from "./create-truncating-collector";
import { spawn } from "child_process";
import * as os from "os";

/**
 * This function should never return a rejected promise: errors should be
 * mapped to a non-zero exit code and the error message should be in stderr.
 */
export function exec(
  command: Array<string>,
  options: SpawnOptions,
  config: AppConfig,
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

  const prog = adaptedCommand[0];
  if (typeof prog !== "string") {
    return Promise.resolve({
      stdout: "",
      stderr: "command[0] is not a string",
      exitCode: 1,
    });
  }

  // We use spawn() instead of exec() or execFile() so that we can set the
  // stdio options to "ignore" for stdin. Ripgrep has a heuristic where it
  // may try to read from stdin as explained here:
  //
  // https://github.com/BurntSushi/ripgrep/blob/e2362d4d5185d02fa857bf381e7bd52e66fafc73/crates/core/flags/hiargs.rs#L1101-L1103
  //
  // This can be a problem because if you save the following to a file and
  // run it with `node`, it will hang forever:
  //
  // ```
  // const {execFile} = require('child_process');
  //
  // execFile('rg', ['foo'], (error, stdout, stderr) => {
  //   if (error) {
  //     console.error(`error: ${error}n\nstderr: ${stderr}`);
  //   } else {
  //     console.log(`stdout: ${stdout}`);
  //   }
  // });
  // ```
  //
  // Even if you pass `{stdio: ["ignore", "pipe", "pipe"] }` to execFile(), the
  // hang still happens as the `stdio` is seemingly ignored. Using spawn()
  // works around this issue.
  const fullOptions: SpawnOptionsWithStdioTuple<
    StdioNull,
    StdioPipe,
    StdioPipe
  > = {
    ...options,
    // Inherit any caller‑supplied stdio flags but force stdin to "ignore" so
    // the child never attempts to read from us (see lengthy comment above).
    stdio: ["ignore", "pipe", "pipe"],
    // Launch the child in its *own* process group so that we can later send a
    // single signal to the entire group – this reliably terminates not only
    // the immediate child but also any grandchildren it might have spawned
    // (think `bash -c "sleep 999"`).
    detached: true,
  };

  const child: ChildProcess = spawn(prog, adaptedCommand.slice(1), fullOptions);
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
    // Get shell output limits from config if available
    const maxBytes = config?.tools?.shell?.maxBytes;
    const maxLines = config?.tools?.shell?.maxLines;

    // Collect stdout and stderr up to configured limits.
    const stdoutCollector = createTruncatingCollector(
      child.stdout!,
      maxBytes,
      maxLines,
    );
    const stderrCollector = createTruncatingCollector(
      child.stderr!,
      maxBytes,
      maxLines,
    );

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
