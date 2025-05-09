import { exec as rawExec } from "../src/utils/agent/sandbox/raw-exec.js";
import { describe, it, expect } from "vitest";
import type { AppConfig } from "src/utils/config.js";

// Import the low‑level exec implementation so we can verify that AbortSignal
// correctly terminates a spawned process. We bypass the higher‑level wrappers
// to keep the test focused and fast.

describe("exec cancellation", () => {
  it("kills the child process when the abort signal is triggered", async () => {
    const abortController = new AbortController();

    // Spawn a node process that would normally run for 5 seconds before
    // printing anything. We should abort long before that happens.
    const cmd = ["node", "-e", "setTimeout(() => console.log('late'), 5000);"];
    const config: AppConfig = {
      model: "test-model",
      instructions: "test-instructions",
    };
    const start = Date.now();

    const promise = rawExec(cmd, {}, config, abortController.signal);

    // Abort almost immediately.
    abortController.abort();

    const result = await promise;
    const durationMs = Date.now() - start;

    // The process should have been terminated rapidly (well under the 5s the
    // child intended to run) – give it a generous 2s budget.
    expect(durationMs).toBeLessThan(2000);

    // Exit code should indicate abnormal termination (anything but zero)
    expect(result.exitCode).not.toBe(0);

    // The child never got a chance to print the word "late".
    expect(result.stdout).not.toContain("late");
  });

  it("allows the process to finish when not aborted", async () => {
    const abortController = new AbortController();

    const config: AppConfig = {
      model: "test-model",
      instructions: "test-instructions",
    };

    const cmd = ["node", "-e", "console.log('finished')"];

    const result = await rawExec(cmd, {}, config, abortController.signal);

    expect(result.exitCode).toBe(0);
    expect(result.stdout.trim()).toBe("finished");
  });
});
