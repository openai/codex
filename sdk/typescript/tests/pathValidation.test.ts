import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { describe, expect, it, beforeEach, afterEach } from "@jest/globals";
import { validateFilePath } from "../src/pathValidation";

describe("validateFilePath", () => {
  let tempDir: string;
  let testFile: string;

  beforeEach(async () => {
    // Create a temporary directory for testing
    tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "path-validation-test-"));
    testFile = path.join(tempDir, "test.txt");
    await fs.writeFile(testFile, "test content");
  });

  afterEach(async () => {
    // Clean up temporary directory
    await fs.rm(tempDir, { recursive: true, force: true });
  });

  it("accepts valid file paths", async () => {
    const result = await validateFilePath(testFile);
    expect(result).toBe(path.resolve(testFile));
  });

  it("rejects paths with null bytes", async () => {
    await expect(validateFilePath("test\0file.txt")).rejects.toThrow(
      "Invalid file path: contains null bytes",
    );
  });

  it("rejects empty paths", async () => {
    await expect(validateFilePath("")).rejects.toThrow("Invalid file path: path is empty");
    await expect(validateFilePath("   ")).rejects.toThrow("Invalid file path: path is empty");
  });

  it("rejects non-existent files", async () => {
    const nonExistentPath = path.join(tempDir, "does-not-exist.txt");
    await expect(validateFilePath(nonExistentPath)).rejects.toThrow(
      "does not exist",
    );
  });

  it("rejects directories", async () => {
    await expect(validateFilePath(tempDir)).rejects.toThrow("is not a file");
  });

  it("blocks access to /etc/passwd", async () => {
    // This test will only pass on Unix-like systems where /etc/passwd exists
    if (process.platform !== "win32") {
      await expect(validateFilePath("/etc/passwd")).rejects.toThrow(
        "Access denied: cannot access sensitive system path",
      );
    }
  });

  it("blocks access to /etc/shadow", async () => {
    if (process.platform !== "win32") {
      await expect(validateFilePath("/etc/shadow")).rejects.toThrow(
        "Access denied: cannot access sensitive system path",
      );
    }
  });

  it("validates paths are within allowed base path", async () => {
    const result = await validateFilePath(testFile, tempDir);
    expect(result).toBe(path.resolve(testFile));
  });

  it("rejects paths outside allowed base path using ..", async () => {
    const outsideDir = await fs.mkdtemp(path.join(os.tmpdir(), "outside-"));
    const outsideFile = path.join(outsideDir, "outside.txt");
    await fs.writeFile(outsideFile, "outside content");

    try {
      // Try to access a file outside the allowed base path
      await expect(validateFilePath(outsideFile, tempDir)).rejects.toThrow(
        "is outside the allowed directory",
      );
    } finally {
      await fs.rm(outsideDir, { recursive: true, force: true });
    }
  });

  it("rejects path traversal attempts with relative paths", async () => {
    // Create a file outside the temp directory
    const outsideDir = await fs.mkdtemp(path.join(os.tmpdir(), "outside-"));
    const outsideFile = path.join(outsideDir, "secret.txt");
    await fs.writeFile(outsideFile, "secret content");

    try {
      // Try to use .. to traverse outside allowed directory
      const maliciousPath = path.join(tempDir, "..", path.basename(outsideDir), "secret.txt");
      await expect(validateFilePath(maliciousPath, tempDir)).rejects.toThrow(
        "is outside the allowed directory",
      );
    } finally {
      await fs.rm(outsideDir, { recursive: true, force: true });
    }
  });

  it("resolves relative paths correctly", async () => {
    const originalCwd = process.cwd();
    try {
      process.chdir(tempDir);
      const relativePath = "./test.txt";
      const result = await validateFilePath(relativePath);
      expect(result).toBe(path.resolve(tempDir, "test.txt"));
    } finally {
      process.chdir(originalCwd);
    }
  });

  it("handles symbolic links securely", async () => {
    if (process.platform !== "win32") {
      // Create a symlink to the test file
      const symlinkPath = path.join(tempDir, "symlink.txt");
      await fs.symlink(testFile, symlinkPath);

      const result = await validateFilePath(symlinkPath);
      // The resolved path should point to the actual file
      expect(result).toBe(path.resolve(testFile));
    }
  });

  it("rejects symlinks pointing outside allowed directory", async () => {
    if (process.platform !== "win32") {
      const outsideDir = await fs.mkdtemp(path.join(os.tmpdir(), "outside-"));
      const outsideFile = path.join(outsideDir, "secret.txt");
      await fs.writeFile(outsideFile, "secret");

      const symlinkPath = path.join(tempDir, "malicious-symlink.txt");
      await fs.symlink(outsideFile, symlinkPath);

      try {
        await expect(validateFilePath(symlinkPath, tempDir)).rejects.toThrow(
          "is outside the allowed directory",
        );
      } finally {
        await fs.rm(outsideDir, { recursive: true, force: true });
      }
    }
  });
});
