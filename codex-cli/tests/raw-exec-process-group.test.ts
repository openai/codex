import { describe, it, expect } from "vitest";
import { exec as rawExec } from "../src/utils/agent/sandbox/raw-exec.js";
import type { AppConfig } from "src/utils/config.js";

// Regression test: When cancelling an in‑flight `rawExec()` the implementation
// must terminate *all* processes that belong to the spawned command – not just
// the direct child.  The original logic only sent `SIGTERM` to the immediate
// child which meant that grandchildren (for instance when running through a
// `bash -c` wrapper) were left running and turned into "zombie" processes.
// Strategy:
//   1. Start a Bash shell that spawns a long‑running `sleep`, prints the PID
//      of that `sleep`, and then waits forever.  This guarantees we can later
//      check if the grand‑child is still alive.
//   2. Abort the exec almost immediately.
//   3. After `rawExec()` resolves we probe the previously printed PID with
//      `process.kill(pid, 0)`.  If the call throws `ESRCH` the process no
//      longer exists – the desired outcome.  Otherwise the test fails.
// The negative‑PID process‑group trick employed by the fixed implementation is
// POSIX‑only.  On Windows we skip the test.

describe("rawExec – abort kills entire process group", () => {
  it("terminates grandchildren spawned via bash", async () => {
    if (process.platform === "win32") {
      return;
    }

    const abortController = new AbortController();
    // Bash script: spawn `sleep 30` in background, print its PID, then wait.
    const script = "sleep 30 & pid=$!; echo $pid; wait $pid";
    const cmd = ["bash", "-c", script];
    const config: AppConfig = {
      model: "test-model",
      instructions: "test-instructions",
    };

    // Start a bash shell that:
    //  - spawns a background `sleep 30`
    //  - prints the PID of the `sleep`
    //  - waits for `sleep` to exit
    const { stdout, exitCode } = await (async () => {
      const p = rawExec(cmd, {}, config, abortController.signal);

      // Give Bash a tiny bit of time to start and print the PID.
      await new Promise((r) => setTimeout(r, 100));

      // Cancel the task – this should kill *both* bash and the inner sleep.
      abortController.abort();

      // Wait for rawExec to resolve after aborting
      return p;
    })();

    // We expect a non‑zero exit code because the process was killed.
    expect(exitCode).not.toBe(0);

    // Extract the PID of the sleep process that bash printed
    const pid = Number(stdout.trim().match(/^\d+/)?.[0]);
    if (pid) {
      // Confirm that the sleep process is no longer alive
      await ensureProcessGone(pid);
    }
  });
});

/**
 * Waits until a process no longer exists, or throws after timeout.
 * @param pid - The process ID to check
 * @throws {Error} If the process is still alive after 500ms
 */
async function ensureProcessGone(pid: number) {
  const timeout = 500;
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    try {
      process.kill(pid, 0); // check if process still exists
      await new Promise((r) => setTimeout(r, 50)); // wait and retry
    } catch (e: any) {
      if (e.code === "ESRCH") {
        return; // process is gone — success
      }
      throw e; // unexpected error — rethrow
    }
  }
  throw new Error(
    `Process with PID ${pid} failed to terminate within ${timeout}ms`,
  );
}
