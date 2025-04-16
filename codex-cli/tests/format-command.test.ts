import { formatCommandForDisplay } from "../src/format-command";
import { describe, test, expect } from "vitest";

describe("formatCommandForDisplay()", () => {
  test("ensure empty string arg appears in output", () => {
    expect(formatCommandForDisplay(["echo", ""])).toEqual("echo ''");
  });

  test("ensure special characters are properly escaped", () => {
    expect(formatCommandForDisplay(["echo", "$HOME"])).toEqual("echo \\$HOME");
  });

  test("ensure quotes are properly escaped", () => {
    expect(formatCommandForDisplay(["echo", "I can't believe this."])).toEqual(
      'echo "I can\'t believe this."',
    );
    expect(
      formatCommandForDisplay(["echo", 'So I said, "No ma\'am!"']),
    ).toEqual('echo "So I said, \\"No ma\'am\\!\\""');
  });
});
