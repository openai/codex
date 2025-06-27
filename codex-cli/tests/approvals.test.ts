import type { SafetyAssessment } from "../src/approvals";

import { canAutoApprove } from "../src/approvals";
import { describe, test, expect } from "vitest";

describe("canAutoApprove()", () => {
  const env = {
    PATH: "/usr/local/bin:/usr/bin:/bin",
    HOME: "/home/user",
  };

  const writeablePaths: Array<string> = [];
  const check = (command: ReadonlyArray<string>): SafetyAssessment =>
    canAutoApprove(
      command,
      /* workdir */ undefined,
      "suggest",
      writeablePaths,
      env,
    );

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
    expect(check(["nl", "-ba", "README.md"])).toEqual({
      type: "auto-approve",
      reason: "View file with line numbers",
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
      reason: "No-op (true)",
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

  test("find", () => {
    expect(check(["find", ".", "-name", "file.txt"])).toEqual({
      type: "auto-approve",
      reason: "Find files or directories",
      group: "Searching",
      runInSandbox: false,
    });

    // Options that can execute arbitrary commands.
    expect(
      check(["find", ".", "-name", "file.txt", "-exec", "rm", "{}", ";"]),
    ).toEqual({
      type: "ask-user",
    });
    expect(
      check(["find", ".", "-name", "*.py", "-execdir", "python3", "{}", ";"]),
    ).toEqual({
      type: "ask-user",
    });
    expect(
      check(["find", ".", "-name", "file.txt", "-ok", "rm", "{}", ";"]),
    ).toEqual({
      type: "ask-user",
    });
    expect(
      check(["find", ".", "-name", "*.py", "-okdir", "python3", "{}", ";"]),
    ).toEqual({
      type: "ask-user",
    });

    // Option that deletes matching files.
    expect(check(["find", ".", "-delete", "-name", "file.txt"])).toEqual({
      type: "ask-user",
    });

    // Options that write pathnames to a file.
    expect(check(["find", ".", "-fls", "/etc/passwd"])).toEqual({
      type: "ask-user",
    });
    expect(check(["find", ".", "-fprint", "/etc/passwd"])).toEqual({
      type: "ask-user",
    });
    expect(check(["find", ".", "-fprint0", "/etc/passwd"])).toEqual({
      type: "ask-user",
    });
    expect(
      check(["find", ".", "-fprintf", "/root/suid.txt", "%#m %u %p\n"]),
    ).toEqual({
      type: "ask-user",
    });
  });

  test("sed", () => {
    // `sed` used to read lines from a file.
    expect(check(["sed", "-n", "1,200p", "filename.txt"])).toEqual({
      type: "auto-approve",
      reason: "Sed print subset",
      group: "Reading files",
      runInSandbox: false,
    });
    // Bad quoting! The model is doing the wrong thing here, so this should not
    // be auto-approved.
    expect(check(["sed", "-n", "'1,200p'", "filename.txt"])).toEqual({
      type: "ask-user",
    });
    // Extra arg: here we are extra conservative, we do not auto-approve.
    expect(check(["sed", "-n", "1,200p", "file1.txt", "file2.txt"])).toEqual({
      type: "ask-user",
    });

    // `sed` used to read lines from a file with a shell command.
    expect(check(["bash", "-lc", "sed -n '1,200p' filename.txt"])).toEqual({
      type: "auto-approve",
      reason: "Sed print subset",
      group: "Reading files",
      runInSandbox: false,
    });

    // Pipe the output of `nl` to `sed`.
    expect(
      check(["bash", "-lc", "nl -ba README.md | sed -n '1,200p'"]),
    ).toEqual({
      type: "auto-approve",
      reason: "View file with line numbers",
      group: "Reading files",
      runInSandbox: false,
    });
  });
});
