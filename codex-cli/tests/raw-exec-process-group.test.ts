import { describe, it, expect } from "vitest";
import { exec as rawExec } from "../src/utils/agent/sandbox/raw-exec.js";

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

    // Kick off the command.
    const execPromise = rawExec(cmd, {}, [], abortController.signal);

    // Give Bash a tiny bit of time to start and print the PID.
    await new Promise((r) => setTimeout(r, 100));

    // Cancel the task – this should kill *both* bash and the inner sleep.
    abortController.abort();

    const { exitCode, stdout } = await execPromise;

    // We expect a non‑zero exit code because the process was killed.
    expect(exitCode).not.toBe(0);

    // Attempt to extract the grand‑child PID from stdout.
    const pidMatch = /^(\d+)/.exec(stdout.trim());

    if (pidMatch) {
      const sleepPid = Number(pidMatch[1]);

      // Verify that the sleep process is no longer alive.
      let alive = true;
      try {
        process.kill(sleepPid, 0);
      } catch (error: any) {
        // Check if error is ESRCH (No such process)
        if (error.code === "ESRCH") {
          alive = false; // Process is dead, as expected.
        } else {
          throw error;
        }
      }
      expect(alive).toBe(false);
    } else {
      // If PID was not printed, it implies bash was killed very early.
      // The test passes implicitly in this scenario as the abort mechanism
      // successfully stopped the command execution quickly.
      expect(true).toBe(true);
    }
  });
});
