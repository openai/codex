import { describe, it, expect } from "vitest";
import { exec as rawExec } from "../src/utils/agent/sandbox/raw-exec.js";

// Regression test for proper process group termination.
// When cancelling an in-flight command, we need to ensure that ALL child processes
// in the process group are terminated, not just the immediate child.
//
// The key issue: If we only kill the immediate child, grandchildren processes
// may continue running in the background as "zombie" processes.
//
// This test verifies that our raw-exec implementation correctly terminates
// all processes in the group by using process.kill(-pid, signal) which sends
// the signal to the entire process group.

describe("rawExec – abort kills entire process group", () => {
  it("terminates grandchildren spawned via bash", async () => {
    if (process.platform === "win32") {
      // The negative-PID process group mechanism isn't supported on Windows
      return;
    }

    const abortController = new AbortController();

    // Create a more complex nested process scenario:
    // 1. A bash process running our script
    // 2. A sleep process that lasts 30 seconds
    // 3. Another child process that runs 'sleep 25'
    // All should be terminated when we abort
    const script = `
      # Start a sleep process and get its PID
      sleep 30 &
      pid1=$!
      
      # Start another child process that itself runs another command
      # This creates a deeper process hierarchy to test group termination
      bash -c "sleep 25 & pid2=\\$!; echo Child2PID=\\$pid2; wait \\$pid2" &
      pid2=$!
      
      # Output the PIDs so we can verify they're killed
      echo "MainSleepPID=$pid1"
      echo "ChildBashPID=$pid2"
      
      # Wait for the first sleep to finish (should be aborted before that)
      wait $pid1
    `;

    const cmd = ["bash", "-c", script];

    // Kick off the command
    const execPromise = rawExec(cmd, {}, [], abortController.signal);

    // Give Bash enough time to start and print the PIDs
    await new Promise((r) => setTimeout(r, 500));

    // Cancel the task - this should kill the entire process group
    abortController.abort();

    const { exitCode, stdout } = await execPromise;

    // We expect a non-zero exit code because the process was killed
    expect(exitCode).not.toBe(0);

    // Extract the PIDs of the child processes
    const mainSleepPidMatch = /MainSleepPID=(\d+)/.exec(stdout);
    const childBashPidMatch = /ChildBashPID=(\d+)/.exec(stdout);
    const childSleepPidMatch = /Child2PID=(\d+)/.exec(stdout);

    // Verify we got at least the main process PID
    if (!mainSleepPidMatch) {
      throw new Error(
        "Failed to get main sleep process PID. Test needs to be adjusted to ensure PID is captured.",
      );
    }

    const mainSleepPid = Number(mainSleepPidMatch[1]);

    // Wait a moment for processes to be terminated completely
    await new Promise((r) => setTimeout(r, 100));

    // Verify the main sleep process was terminated
    let mainProcessAlive = true;
    try {
      process.kill(mainSleepPid, 0);
    } catch (error: any) {
      if (error.code === "ESRCH") {
        mainProcessAlive = false; // Process is dead, as expected
      }
    }
    expect(mainProcessAlive).toBe(false);

    // If we captured the second-level child process PID, verify it was also terminated
    if (childBashPidMatch) {
      const childBashPid = Number(childBashPidMatch[1]);
      let childBashAlive = true;
      try {
        process.kill(childBashPid, 0);
      } catch (error: any) {
        if (error.code === "ESRCH") {
          childBashAlive = false; // Process is dead, as expected
        }
      }
      expect(childBashAlive).toBe(false);
    }

    // If we captured the third-level child process PID, verify it was also terminated
    if (childSleepPidMatch) {
      const childSleepPid = Number(childSleepPidMatch[1]);
      let childSleepAlive = true;
      try {
        process.kill(childSleepPid, 0);
      } catch (error: any) {
        if (error.code === "ESRCH") {
          childSleepAlive = false; // Process is dead, as expected
        }
      }
      expect(childSleepAlive).toBe(false);
    }
  });
});
