import type { SafetyAssessment } from "../src/approvals";

import { canAutoApprove } from "../src/approvals";
import { describe, test, expect } from "vitest";

describe("canAutoApprove()", () => {
  const env = {
    PATH: "/usr/local/bin:/usr/bin:/bin",
    HOME: "/home/user",
  };

  const writeablePaths: Array<string> = [];
  const check = (
    command: ReadonlyArray<string>,
    policy: "suggest" | "auto-edit" | "full-auto" = "suggest",
  ): SafetyAssessment => canAutoApprove(command, policy, writeablePaths, env);

  test("simple commands in suggest mode should require approval", () => {
    // In suggest mode, all commands should require approval
    expect(check(["ls"])).toEqual({ type: "ask-user" });
    expect(check(["cat", "file.txt"])).toEqual({ type: "ask-user" });
    expect(check(["pwd"])).toEqual({ type: "ask-user" });
  });

  test("simple safe commands in auto-edit mode", () => {
    // In auto-edit mode, safe commands should be auto-approved
    expect(check(["ls"], "auto-edit")).toEqual({
      type: "auto-approve",
      reason: "List directory",
      group: "Searching",
      runInSandbox: false,
    });
    expect(check(["cat", "file.txt"], "auto-edit")).toEqual({
      type: "auto-approve",
      reason: "View file contents",
      group: "Reading files",
      runInSandbox: false,
    });
    expect(check(["pwd"], "auto-edit")).toEqual({
      type: "auto-approve",
      reason: "Print working directory",
      group: "Navigating",
      runInSandbox: false,
    });
  });

  test("bash commands in suggest mode should require approval", () => {
    // In suggest mode, all bash commands should require approval
    expect(check(["bash", "-lc", "ls"])).toEqual({ type: "ask-user" });
    expect(check(["bash", "-lc", "ls $HOME"])).toEqual({ type: "ask-user" });
    expect(check(["bash", "-lc", "git show ab9811cb90"])).toEqual({
      type: "ask-user",
    });
  });

  test("bash commands in auto-edit mode", () => {
    // In auto-edit mode, safe bash commands should be auto-approved
    expect(check(["bash", "-lc", "ls"], "auto-edit")).toEqual({
      type: "auto-approve",
      reason: "List directory",
      group: "Searching",
      runInSandbox: false,
    });
    expect(check(["bash", "-lc", "ls $HOME"], "auto-edit")).toEqual({
      type: "auto-approve",
      reason: "List directory",
      group: "Searching",
      runInSandbox: false,
    });
    expect(check(["bash", "-lc", "git show ab9811cb90"], "auto-edit")).toEqual({
      type: "auto-approve",
      reason: "Git show",
      group: "Using git",
      runInSandbox: false,
    });
  });

  test("bash -lc commands with unsafe redirects", () => {
    // In suggest mode, all commands should require approval
    expect(check(["bash", "-lc", "echo hello > file.txt"])).toEqual({
      type: "ask-user",
    });
    expect(check(["bash", "-lc", "ls && pwd"])).toEqual({
      type: "ask-user",
    });

    // In auto-edit mode, commands with redirects should still require approval
    expect(
      check(["bash", "-lc", "echo hello > file.txt"], "auto-edit"),
    ).toEqual({
      type: "ask-user",
    });

    // In auto-edit mode, safe commands with safe operators should be auto-approved
    expect(check(["bash", "-lc", "ls && pwd"], "auto-edit")).toEqual({
      type: "auto-approve",
      reason: "List directory",
      group: "Searching",
      runInSandbox: false,
    });
  });

  test("true command in suggest mode requires approval", () => {
    expect(check(["true"])).toEqual({ type: "ask-user" });
  });

  test("true command in auto-edit mode is auto-approved", () => {
    expect(check(["true"], "auto-edit")).toEqual({
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
});
