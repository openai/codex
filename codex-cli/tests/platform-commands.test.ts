import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { adaptCommandForPlatform } from "../src/utils/agent/platform-commands.js";
import * as path from "path";
import * as fs from "fs";

// Mock the platform and filesystem modules
vi.mock("path", () => ({
  join: vi.fn(),
}));

vi.mock("fs", () => ({
  existsSync: vi.fn(),
  mkdirSync: vi.fn(),
}));

describe("platform-commands", () => {
  const originalPlatform = process.platform;
  const mockCwd = "/mock/cwd";
  const mockTmpDir = "/mock/cwd/.tmp";

  beforeEach(() => {
    // Reset all mocks
    vi.resetAllMocks();

    // Mock process.cwd()
    vi.spyOn(process, "cwd").mockReturnValue(mockCwd);

    // Mock path.join
    vi.mocked(path.join).mockReturnValue(mockTmpDir);
  });

  afterEach(() => {
    // Restore the original platform
    Object.defineProperty(process, "platform", {
      value: originalPlatform,
    });
  });

  describe("adaptCommandForPlatform", () => {
    describe("Windows platform adaptations", () => {
      beforeEach(() => {
        // Mock Windows platform
        Object.defineProperty(process, "platform", {
          value: "win32",
        });
      });

      it("should adapt ls command to dir on Windows", () => {
        const command = ["ls"];
        const adapted = adaptCommandForPlatform(command);
        expect(adapted).toEqual(["dir"]);
      });

      it("should adapt ls -l to dir /p on Windows", () => {
        const command = ["ls", "-l"];
        const adapted = adaptCommandForPlatform(command);
        expect(adapted).toEqual(["dir", "/p"]);
      });

      it("should not modify commands that don't need adaptation", () => {
        const command = ["echo", "hello"];
        const adapted = adaptCommandForPlatform(command);
        expect(adapted).toEqual(["echo", "hello"]);
      });

      it("should handle empty command arrays", () => {
        const command: Array<string> = [];
        const adapted = adaptCommandForPlatform(command);
        expect(adapted).toEqual([]);
      });
    });

    describe("macOS Go command adaptations", () => {
      beforeEach(() => {
        // Mock macOS platform
        Object.defineProperty(process, "platform", {
          value: "darwin",
        });
      });

      it("should adapt go build command on macOS", () => {
        // Mock fs.existsSync to return false (directory doesn't exist)
        vi.mocked(fs.existsSync).mockReturnValue(false);

        const command = ["go", "build"];
        const adapted = adaptCommandForPlatform(command);

        // Verify tmp directory creation was attempted
        expect(fs.existsSync).toHaveBeenCalledWith(mockTmpDir);
        expect(fs.mkdirSync).toHaveBeenCalledWith(mockTmpDir, {
          recursive: true,
        });

        // Verify command adaptation
        expect(adapted).toEqual([
          "env",
          `GOTMPDIR=${mockTmpDir}`,
          "go",
          "build",
        ]);
      });

      it("should not create tmp directory if it already exists", () => {
        // Mock fs.existsSync to return true (directory exists)
        vi.mocked(fs.existsSync).mockReturnValue(true);

        const command = ["go", "run", "main.go"];
        const adapted = adaptCommandForPlatform(command);

        // Verify tmp directory creation was not attempted
        expect(fs.existsSync).toHaveBeenCalledWith(mockTmpDir);
        expect(fs.mkdirSync).not.toHaveBeenCalled();

        // Verify command adaptation
        expect(adapted).toEqual([
          "env",
          `GOTMPDIR=${mockTmpDir}`,
          "go",
          "run",
          "main.go",
        ]);
      });

      it("should not adapt non-Go commands on macOS", () => {
        const command = ["echo", "hello"];
        const adapted = adaptCommandForPlatform(command);

        // Verify no tmp directory checks or creation
        expect(fs.existsSync).not.toHaveBeenCalled();
        expect(fs.mkdirSync).not.toHaveBeenCalled();

        // Command should be unchanged
        expect(adapted).toEqual(["echo", "hello"]);
      });
    });

    describe("Linux platform", () => {
      beforeEach(() => {
        // Mock Linux platform
        Object.defineProperty(process, "platform", {
          value: "linux",
        });
      });

      it("should not adapt commands on Linux", () => {
        const command = ["ls", "-l"];
        const adapted = adaptCommandForPlatform(command);
        expect(adapted).toEqual(["ls", "-l"]);
      });

      it("should not adapt Go commands on Linux", () => {
        const command = ["go", "build"];
        const adapted = adaptCommandForPlatform(command);

        // Verify no tmp directory checks or creation
        expect(fs.existsSync).not.toHaveBeenCalled();
        expect(fs.mkdirSync).not.toHaveBeenCalled();

        // Command should be unchanged
        expect(adapted).toEqual(["go", "build"]);
      });
    });
  });
});
