import { describe, it, expect } from "vitest";
import { parse } from "shell-quote";

/* eslint-disable no-console */

describe("shell-quote parse with pipes", () => {
  it("should correctly parse a command with a pipe", () => {
    const cmd = 'grep -n "finally:" some-file | head';
    const tokens = parse(cmd);
    console.log("Parsed tokens:", JSON.stringify(tokens, null, 2));

    // Check if any token has an 'op' property
    const hasOpToken = tokens.some(
      (token) => typeof token === "object" && "op" in token,
    );

    expect(hasOpToken).toBe(true);
  });
});
