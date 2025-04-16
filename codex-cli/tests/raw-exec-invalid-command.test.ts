import { describe, it, expect } from "vitest";

// Import the low‑level exec implementation directly so the test focuses on
// the child‑process handling without involving higher‑level wrappers.
import { exec as rawExec } from "../src/utils/agent/sandbox/raw-exec.js";

describe("rawExec – invalid command handling", () => {
  it("gracefully resolves when the executable cannot be spawned", async () => {
    // Use an obviously non‑existent program name to guarantee ENOENT on all
    // platforms.
    const cmd = ["definitely-not-a-command-1234567890"];

    const result = await rawExec(cmd, {}, []);

    // The promise should resolve (i.e. not throw) and return a non‑zero exit
    // code so that the caller can react appropriately.
    expect(result.exitCode).not.toBe(0);

    // stderr should contain some information about the failure.
    expect(result.stderr.length).toBeGreaterThan(0);
  });
});
