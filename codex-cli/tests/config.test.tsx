import type * as fsType from "fs";

import {
  loadConfig,
  saveConfig,
  loadProvidersFromFile,
  getMergedProviders,
  PROVIDERS_CONFIG_PATH,
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
      // no-op in in‑memory store
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

// Mock the providers import to control the default providers
vi.mock("../src/utils/providers.js", () => {
  return {
    providers: {
      openai: {
        name: "OpenAI",
        baseURL: "https://api.openai.com/v1",
        envKey: "OPENAI_API_KEY",
      },
      mistral: {
        name: "Mistral",
        baseURL: "https://api.mistral.ai/v1",
        envKey: "MISTRAL_API_KEY",
      },
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

// Test for custom providers loading from providers.json
test("loads providers from providers.json when file exists", () => {
  // Setup providers.json file
  const customProviders = {
    openai: {
      name: "Custom OpenAI",
      baseURL: "https://custom-api.openai.com/v1",
      envKey: "CUSTOM_OPENAI_API_KEY",
    },
    groq: {
      name: "Groq",
      baseURL: "https://api.groq.com/v1",
      envKey: "GROQ_API_KEY",
    },
  };

  // Add providers.json to memory filesystem
  memfs[PROVIDERS_CONFIG_PATH] = JSON.stringify(customProviders, null, 2);

  // Test loadProvidersFromFile function
  const loadedProviders = loadProvidersFromFile();

  // Verify loaded providers match our custom configuration
  expect(loadedProviders["openai"]?.name).toBe("Custom OpenAI");
  expect(loadedProviders["openai"]?.baseURL).toBe(
    "https://custom-api.openai.com/v1",
  );
  expect(loadedProviders["openai"]?.envKey).toBe("CUSTOM_OPENAI_API_KEY");
  expect(loadedProviders["groq"]?.name).toBe("Groq");
});

// Test for merging default and custom providers
test("merges default and custom providers properly", () => {
  // Setup providers.json with only one provider that overrides a default
  const customProviders = {
    openai: {
      name: "Custom OpenAI",
      baseURL: "https://custom-api.openai.com/v1",
      envKey: "CUSTOM_OPENAI_API_KEY",
    },
  };

  // Add providers.json to memory filesystem
  memfs[PROVIDERS_CONFIG_PATH] = JSON.stringify(customProviders, null, 2);

  // Get merged providers
  const mergedProviders = getMergedProviders();

  // Verify the custom provider overrides the default
  expect(mergedProviders["openai"]?.name).toBe("Custom OpenAI");
  expect(mergedProviders["openai"]?.baseURL).toBe(
    "https://custom-api.openai.com/v1",
  );

  // Verify default providers are still available
  expect(mergedProviders["mistral"]).toBeDefined();
  expect(mergedProviders["mistral"]?.name).toBe("Mistral");
});

// Test when providers.json doesn't exist
test("uses default providers when providers.json doesn't exist", () => {
  // Ensure providers.json doesn't exist
  expect(memfs[PROVIDERS_CONFIG_PATH]).toBeUndefined();

  // Get merged providers
  const mergedProviders = getMergedProviders();

  // Verify default providers are used
  expect(mergedProviders["openai"]?.name).toBe("OpenAI");
  expect(mergedProviders["openai"]?.baseURL).toBe("https://api.openai.com/v1");
  expect(mergedProviders["mistral"]?.name).toBe("Mistral");
});

// Test error handling when providers.json is invalid
test("handles invalid providers.json gracefully", () => {
  // Setup invalid JSON in providers.json
  memfs[PROVIDERS_CONFIG_PATH] = "{invalid-json}";

  // Mock console.error to prevent test output pollution
  const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

  // Should return empty object when JSON parsing fails
  const loadedProviders = loadProvidersFromFile();

  // Verify error was logged
  expect(consoleSpy).toHaveBeenCalled();
  expect(loadedProviders).toEqual({});

  // Get merged providers - should fall back to defaults
  const mergedProviders = getMergedProviders();

  // Verify default providers are used
  expect(mergedProviders["openai"]?.name).toBe("OpenAI");

  // Restore console.error
  consoleSpy.mockRestore();
});
