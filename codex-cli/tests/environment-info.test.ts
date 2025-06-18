import { getEnvironmentInfo } from "../src/utils/platform-info.js";
import { test, expect } from "vitest";
import os from "os";
import path from "path";

test("reports platform and shell", () => {
  const info = getEnvironmentInfo();
  const expectedPlatform = `${os.platform()} ${os.arch()} ${os.release()}`;
  const shellPath = process.env["SHELL"] || process.env["ComSpec"] || "";
  const expectedShell = shellPath ? path.basename(shellPath) : "unknown";
  expect(info.platform).toBe(expectedPlatform);
  expect(info.shell).toBe(expectedShell);
});
