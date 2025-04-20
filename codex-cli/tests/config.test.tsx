import type * as fsType from "fs";

import {
  loadConfig,
  saveConfig,
  saveRepoConfig,
  discoverRepoConfigPath,
  REPO_CONFIG_JSON_FILEPATH,
} from "../src/utils/config.js"; // parent import first
import { AutoApprovalMode } from "../src/utils/auto-approval-mode.js";
import { tmpdir } from "os";
import { join } from "path";
import { test, expect, beforeEach, afterEach, vi } from "vitest";

// In‑memory FS store
let memfs: Record<string, string> = {};

// Mock out the parts of "fs" that our config module uses:
vi.mock("fs", async () => {
  // now `real` is the actual fs module
  const real = (await vi.importActual("fs")) as typeof fsType;
  return {
    ...real,
    existsSync: (path: string) => memfs[path] !== undefined,
    readFileSync: (path: string) => {
      if (memfs[path] === undefined) {
        throw new Error("ENOENT");
      }
      return memfs[path];
    },
    writeFileSync: (path: string, data: string) => {
      memfs[path] = data;
    },
    mkdirSync: () => {
      // no‑op in in‑memory store
    },
    rmSync: (path: string) => {
      // recursively delete any key under this prefix
      const prefix = path.endsWith("/") ? path : path + "/";
      for (const key of Object.keys(memfs)) {
        if (key === path || key.startsWith(prefix)) {
          delete memfs[key];
        }
      }
    },
  };
});

let testDir: string;
let testConfigPath: string;
let testInstructionsPath: string;

beforeEach(() => {
  memfs = {}; // reset in‑memory store
  testDir = tmpdir(); // use the OS temp dir as our "cwd"
  testConfigPath = join(testDir, "config.json");
  testInstructionsPath = join(testDir, "instructions.md");
});

afterEach(() => {
  memfs = {};
});

test("loads default config if files don't exist", () => {
  const config = loadConfig(testConfigPath, testInstructionsPath, {
    disableProjectDoc: true,
  });
  // Keep the test focused on just checking that default model and instructions are loaded
  // so we need to make sure we check just these properties
  expect(config.model).toBe("o4-mini");
  expect(config.instructions).toBe("");
});

test("saves and loads config correctly", () => {
  const testConfig = {
    model: "test-model",
    instructions: "test instructions",
    notify: false,
  };
  saveConfig(testConfig, testConfigPath, testInstructionsPath);

  // Our in‑memory fs should now contain those keys:
  expect(memfs[testConfigPath]).toContain(`"model": "test-model"`);
  expect(memfs[testInstructionsPath]).toBe("test instructions");

  const loadedConfig = loadConfig(testConfigPath, testInstructionsPath, {
    disableProjectDoc: true,
  });
  // Check just the specified properties that were saved
  expect(loadedConfig.model).toBe(testConfig.model);
  expect(loadedConfig.instructions).toBe(testConfig.instructions);
});

test("loads user instructions + project doc when codex.md is present", () => {
  // 1) seed memfs: a config JSON, an instructions.md, and a codex.md in the cwd
  const userInstr = "here are user instructions";
  const projectDoc = "# Project Title\n\nSome project‑specific doc";
  // first, make config so loadConfig will see storedConfig
  memfs[testConfigPath] = JSON.stringify({ model: "mymodel" }, null, 2);
  // then user instructions:
  memfs[testInstructionsPath] = userInstr;
  // and now our fake codex.md in the cwd:
  const codexPath = join(testDir, "codex.md");
  memfs[codexPath] = projectDoc;

  // 2) loadConfig without disabling project‑doc, but with cwd=testDir
  const cfg = loadConfig(testConfigPath, testInstructionsPath, {
    cwd: testDir,
  });

  // 3) assert we got both pieces concatenated
  expect(cfg.model).toBe("mymodel");
  expect(cfg.instructions).toBe(
    userInstr + "\n\n--- project-doc ---\n\n" + projectDoc,
  );
});

test("loads and saves approvalMode correctly", () => {
  // Setup config with approvalMode
  memfs[testConfigPath] = JSON.stringify(
    {
      model: "mymodel",
      approvalMode: AutoApprovalMode.AUTO_EDIT,
    },
    null,
    2,
  );
  memfs[testInstructionsPath] = "test instructions";

  // Load config and verify approvalMode
  const loadedConfig = loadConfig(testConfigPath, testInstructionsPath, {
    disableProjectDoc: true,
  });

  // Check approvalMode was loaded correctly
  expect(loadedConfig.approvalMode).toBe(AutoApprovalMode.AUTO_EDIT);

  // Modify approvalMode and save
  const updatedConfig = {
    ...loadedConfig,
    approvalMode: AutoApprovalMode.FULL_AUTO,
  };

  saveConfig(updatedConfig, testConfigPath, testInstructionsPath);

  // Verify saved config contains updated approvalMode
  expect(memfs[testConfigPath]).toContain(
    `"approvalMode": "${AutoApprovalMode.FULL_AUTO}"`,
  );

  // Load again and verify updated value
  const reloadedConfig = loadConfig(testConfigPath, testInstructionsPath, {
    disableProjectDoc: true,
  });
  expect(reloadedConfig.approvalMode).toBe(AutoApprovalMode.FULL_AUTO);
});

test("discovers repository config in current directory", () => {
  // Create a repository config file in the current directory
  const repoConfigPath = join(testDir, REPO_CONFIG_JSON_FILEPATH);
  memfs[repoConfigPath] = JSON.stringify({ model: "repo-model" }, null, 2);

  // Discover the repository config path
  const discoveredPath = discoverRepoConfigPath(testDir);

  // Verify the discovered path matches the expected path
  expect(discoveredPath).toBe(repoConfigPath);
});

test("discovers repository config in git root", () => {
  // Create a git directory to simulate a git repository
  const gitDir = join(testDir, ".git");
  memfs[gitDir] = "";

  // Create a repository config file in the git root
  const repoConfigPath = join(testDir, REPO_CONFIG_JSON_FILEPATH);
  memfs[repoConfigPath] = JSON.stringify({ model: "repo-model" }, null, 2);

  // Create a subdirectory
  const subDir = join(testDir, "subdir");

  // Discover the repository config path from the subdirectory
  const discoveredPath = discoverRepoConfigPath(subDir);

  // Verify the discovered path matches the expected path in the git root
  expect(discoveredPath).toBe(repoConfigPath);
});

test("saves repository config correctly", () => {
  const testConfig = {
    model: "repo-specific-model",
    instructions: "repo instructions",
    notify: true,
    approvalMode: AutoApprovalMode.FULL_AUTO,
  };

  // Save repository config
  saveRepoConfig(testConfig, testDir);

  // Check that the repository config file was created
  const repoConfigPath = join(testDir, REPO_CONFIG_JSON_FILEPATH);
  expect(memfs[repoConfigPath]).toBeDefined();

  // Verify the content of the repository config file
  const savedConfig = JSON.parse(memfs[repoConfigPath] || "{}");
  expect(savedConfig.model).toBe(testConfig.model);
  expect(savedConfig.approvalMode).toBe(testConfig.approvalMode);
});

test("repository config takes precedence over global config", () => {
  // Setup global config
  memfs[testConfigPath] = JSON.stringify(
    {
      model: "global-model",
      approvalMode: AutoApprovalMode.SUGGEST,
      safeCommands: ["npm test"],
    },
    null,
    2,
  );

  // Setup repository config
  const repoConfigPath = join(testDir, REPO_CONFIG_JSON_FILEPATH);
  memfs[repoConfigPath] = JSON.stringify(
    {
      model: "repo-model",
      approvalMode: AutoApprovalMode.FULL_AUTO,
      safeCommands: ["npm run build"],
    },
    null,
    2,
  );

  // Load config
  const config = loadConfig(testConfigPath, testInstructionsPath, {
    cwd: testDir,
    disableProjectDoc: true,
  });

  // Verify that repository config values take precedence
  expect(config.model).toBe("repo-model");
  expect(config.approvalMode).toBe(AutoApprovalMode.FULL_AUTO);

  // Verify that safeCommands are merged
  expect(config.safeCommands).toContain("npm test");
  expect(config.safeCommands).toContain("npm run build");
  expect(config.safeCommands?.length).toBe(2);
});

test("disableRepoConfig option prevents loading repository config", () => {
  // Setup global config
  memfs[testConfigPath] = JSON.stringify(
    {
      model: "global-model",
      approvalMode: AutoApprovalMode.SUGGEST,
    },
    null,
    2,
  );

  // Setup repository config
  const repoConfigPath = join(testDir, REPO_CONFIG_JSON_FILEPATH);
  memfs[repoConfigPath] = JSON.stringify(
    {
      model: "repo-model",
      approvalMode: AutoApprovalMode.FULL_AUTO,
    },
    null,
    2,
  );

  // Load config with disableRepoConfig option
  const config = loadConfig(testConfigPath, testInstructionsPath, {
    cwd: testDir,
    disableProjectDoc: true,
    disableRepoConfig: true,
  });

  // Verify that global config values are used
  expect(config.model).toBe("global-model");
  expect(config.approvalMode).toBe(AutoApprovalMode.SUGGEST);
});
