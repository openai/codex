import { describe, it, expect } from "vitest";
import { parse } from "shell-quote";

// The fixed requiresShell function
function requiresShell(cmd: Array<string>): boolean {
  // If the command is a single string that contains shell operators,
  // it needs to be run with shell: true
  if (cmd.length === 1 && cmd[0] !== undefined) {
    const tokens = parse(cmd[0]) as Array<any>;
    return tokens.some((token) => typeof token === "object" && "op" in token);
  }

  // If the command is split into multiple arguments, we don't need shell: true
  // even if one of the arguments is a shell operator like '|'
  return false;
}

describe("fixed requiresShell function", () => {
  it("should detect pipe in a single argument", () => {
    const cmd = ['grep -n "finally:" some-file | head'];
    expect(requiresShell(cmd)).toBe(true);
  });

  it("should not detect pipe in separate arguments", () => {
    const cmd = ["grep", "-n", "finally:", "some-file", "|", "head"];
    expect(requiresShell(cmd)).toBe(false);
  });

  it("should handle other shell operators in a single argument", () => {
    const cmd = ["echo hello && echo world"];
    expect(requiresShell(cmd)).toBe(true);
  });

  it("should not enable shell for normal commands", () => {
    const cmd = ["ls", "-la"];
    expect(requiresShell(cmd)).toBe(false);
  });
});
