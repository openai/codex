import { describe, it, expect } from "vitest";
import { parse } from "shell-quote";

/* eslint-disable no-console */

// Recreate the requiresShell function for testing
function requiresShell(cmd: Array<string>): boolean {
  // If the command is a single string that contains shell operators,
  // it needs to be run with shell: true
  if (cmd.length === 1 && cmd[0] !== undefined) {
    const tokens = parse(cmd[0]) as Array<any>;
    console.log(
      `Parsing argument: "${cmd[0]}", tokens:`,
      JSON.stringify(tokens, null, 2),
    );
    return tokens.some((token) => typeof token === "object" && "op" in token);
  }

  // If the command is split into multiple arguments, we don't need shell: true
  // even if one of the arguments is a shell operator like '|'
  cmd.forEach((arg) => {
    const tokens = parse(arg) as Array<any>;
    console.log(
      `Parsing argument: "${arg}", tokens:`,
      JSON.stringify(tokens, null, 2),
    );
  });
  console.log("Result for separate arguments: false");
  return false;
}

describe("requiresShell function", () => {
  it("should detect pipe in a single argument", () => {
    const cmd = ['grep -n "finally:" some-file | head'];
    expect(requiresShell(cmd)).toBe(true);
  });

  it("should not detect pipe in separate arguments", () => {
    const cmd = ["grep", "-n", "finally:", "some-file", "|", "head"];
    const result = requiresShell(cmd);
    console.log("Result for separate arguments:", result);
    expect(result).toBe(false);
  });

  it("should handle other shell operators", () => {
    const cmd = ["echo hello && echo world"];
    expect(requiresShell(cmd)).toBe(true);
  });
});
