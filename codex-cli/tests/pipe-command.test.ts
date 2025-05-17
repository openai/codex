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

  it("should parse multiple pipes in a command", () => {
    const cmd = "cat a.txt | grep foo | sort | uniq";
    const tokens = parse(cmd);
    const opTokens = tokens.filter((t) => typeof t === "object" && "op" in t);
    expect(opTokens.length).toBe(3);
    expect(opTokens.every((t) => t.op === "|")).toBe(true);
  });

  it("should parse pipes with or without spaces", () => {
    const cmd = "echo foo|grep foo";
    const tokens = parse(cmd);
    const opToken = tokens.find((t) => typeof t === "object" && "op" in t);
    expect(opToken && opToken.op).toBe("|");
  });

  it("should parse pipe and other operators together", () => {
    const cmd = "echo foo && cat bar | grep baz || echo done";
    const tokens = parse(cmd);
    const ops = tokens
      .filter((t) => typeof t === "object" && "op" in t)
      .map((t) => t.op);
    expect(ops).toEqual(["&&", "|", "||"]);
  });
});
