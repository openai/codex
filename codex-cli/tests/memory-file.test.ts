import { test, expect, vi } from "vitest";
import fs from "fs";
import path from "path";
import os from "os";

import {
  loadConfig,
  saveConfig,
  appendMemoryFile,
  getMemoryFilePath,
} from "../src/utils/config";

test("memory file append and load", () => {
  // Create a temporary home directory to isolate config
  const tmpHome = fs.mkdtempSync(path.join(os.tmpdir(), "codex-test-home-"));
  vi.spyOn(os, "homedir").mockReturnValue(tmpHome);

  // Create a temporary project directory
  const tmpProject = fs.mkdtempSync(path.join(os.tmpdir(), "codex-test-proj-"));

  // Initial load to create default config
  const initialConfig = loadConfig(undefined, undefined, { cwd: tmpProject });
  // Enable memory in config and save
  initialConfig.memory = { enabled: true };
  saveConfig(initialConfig);

  // Append a memory entry
  const testEntry = "Ran ls and saw file1.txt file2.txt";
  appendMemoryFile(tmpProject, testEntry);

  // The memory file should exist and contain the entry
  const memPath = getMemoryFilePath(tmpProject);
  expect(fs.existsSync(memPath)).toBe(true);
  const memContent = fs.readFileSync(memPath, "utf-8");
  expect(memContent).toContain(testEntry + "\n");

  // Reload config with memory enabled, and ensure instructions start with the memory
  const loadedConfig = loadConfig(undefined, undefined, { cwd: tmpProject });
  expect(loadedConfig.memory?.enabled).toBe(true);
  expect(loadedConfig.instructions.startsWith(testEntry)).toBe(true);
});
