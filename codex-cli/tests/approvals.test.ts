import type { SafetyAssessment } from "../src/approvals";

import { canAutoApprove } from "../src/approvals";
import { describe, test, expect, vi } from "vitest";

vi.mock("../src/utils/config", () => ({
  loadConfig: () => ({
    safeCommands: ["npm test", "sl"],
  }),
}));

describe("canAutoApprove()", () => {
  const env = {
    PATH: "/usr/local/bin:/usr/bin:/bin",
    HOME: "/home/user",
  };

  const writeablePaths: Array<string> = [];
  const check = (command: ReadonlyArray<string>): SafetyAssessment =>
    canAutoApprove(command, "suggest", writeablePaths, env);

  test("simple safe commands", () => {
    expect(check(["ls"])).toEqual({
      type: "auto-approve",
      reason: "List directory",
      group: "Searching",
      runInSandbox: false,
    });
    expect(check(["cat", "file.txt"])).toEqual({
      type: "auto-approve",
      reason: "View file contents",
      group: "Reading files",
      runInSandbox: false,
    });
    expect(check(["pwd"])).toEqual({
      type: "auto-approve",
      reason: "Print working directory",
      group: "Navigating",
      runInSandbox: false,
    });
  });

  test("simple safe commands within a `bash -lc` call", () => {
    expect(check(["bash", "-lc", "ls"])).toEqual({
      type: "auto-approve",
      reason: "List directory",
      group: "Searching",
      runInSandbox: false,
    });
    expect(check(["bash", "-lc", "ls $HOME"])).toEqual({
      type: "auto-approve",
      reason: "List directory",
      group: "Searching",
      runInSandbox: false,
    });
    expect(check(["bash", "-lc", "git show ab9811cb90"])).toEqual({
      type: "auto-approve",
      reason: "Git show",
      group: "Using git",
      runInSandbox: false,
    });
  });

  test("bash -lc commands with unsafe redirects", () => {
    expect(check(["bash", "-lc", "echo hello > file.txt"])).toEqual({
      type: "ask-user",
    });
    // In theory, we could make our checker more sophisticated to auto-approve
    // This previously required approval, but now that we consider safe
    // operators like "&&" the entire expression can be auto‑approved.
    expect(check(["bash", "-lc", "ls && pwd"])).toEqual({
      type: "auto-approve",
      reason: "List directory",
      group: "Searching",
      runInSandbox: false,
    });
  });

  test("true command is considered safe", () => {
    expect(check(["true"])).toEqual({
      type: "auto-approve",
      reason: "No‑op (true)",
      group: "Utility",
      runInSandbox: false,
    });
  });

  test("commands that should require approval", () => {
    // Should this be on the auto-approved list?
    expect(check(["printenv"])).toEqual({ type: "ask-user" });

    expect(check(["git", "commit"])).toEqual({ type: "ask-user" });

    expect(check(["pytest"])).toEqual({ type: "ask-user" });

    expect(check(["cargo", "build"])).toEqual({ type: "ask-user" });
  });

  test("commands in safeCommands config should be safe", async () => {
    expect(check(["npm", "test"])).toEqual({
      type: "auto-approve",
      reason: "User-defined safe command",
      group: "User config",
      runInSandbox: false,
    });

    expect(check(["sl"])).toEqual({
      type: "auto-approve",
      reason: "User-defined safe command",
      group: "User config",
      runInSandbox: false,
    });

    expect(check(["npm", "test", "--watch"])).toEqual({
      type: "auto-approve",
      reason: "User-defined safe command",
      group: "User config",
      runInSandbox: false,
    });
  });
});
