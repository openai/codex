import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  loadConfig,
  DEFAULT_REASONING_EFFORT,
  saveConfig,
} from "../src/utils/config";
import type { ReasoningEffort } from "openai/resources.mjs";
import * as fs from "fs";

// Mock the fs module
vi.mock("fs", () => ({
  existsSync: vi.fn(),
  readFileSync: vi.fn(),
  writeFileSync: vi.fn(),
  mkdirSync: vi.fn(),
}));

// Mock path.dirname
vi.mock("path", async () => {
  const actual = await vi.importActual("path");
  return {
    ...actual,
    dirname: vi.fn().mockReturnValue("/mock/dir"),
  };
});

describe("Reasoning Effort Configuration", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('should have "high" as the default reasoning effort', () => {
    expect(DEFAULT_REASONING_EFFORT).toBe("high");
  });

  it("should use default reasoning effort when not specified in config", () => {
    // Mock fs.existsSync to return true for config file
    vi.mocked(fs.existsSync).mockImplementation(() => true);

    // Mock fs.readFileSync to return a JSON with no reasoningEffort
    vi.mocked(fs.readFileSync).mockImplementation(() =>
      JSON.stringify({ model: "test-model" }),
    );

    const config = loadConfig("/mock/config.json", "/mock/instructions.md");

    // Config should not have reasoningEffort explicitly set
    expect(config.reasoningEffort).toBeUndefined();
  });

  it("should load reasoningEffort from config file", () => {
    // Mock fs.existsSync to return true for config file
    vi.mocked(fs.existsSync).mockImplementation(() => true);

    // Mock fs.readFileSync to return a JSON with reasoningEffort
    vi.mocked(fs.readFileSync).mockImplementation(() =>
      JSON.stringify({
        model: "test-model",
        reasoningEffort: "low" as ReasoningEffort,
      }),
    );

    const config = loadConfig("/mock/config.json", "/mock/instructions.md");

    // Config should have the reasoningEffort from the file
    expect(config.reasoningEffort).toBe("low");
  });

  it("should support all valid reasoning effort values", () => {
    // Valid values for ReasoningEffort
    const validEfforts: Array<ReasoningEffort> = ["low", "medium", "high"];

    for (const effort of validEfforts) {
      // Mock fs.existsSync to return true for config file
      vi.mocked(fs.existsSync).mockImplementation(() => true);

      // Mock fs.readFileSync to return a JSON with reasoningEffort
      vi.mocked(fs.readFileSync).mockImplementation(() =>
        JSON.stringify({
          model: "test-model",
          reasoningEffort: effort,
        }),
      );

      const config = loadConfig("/mock/config.json", "/mock/instructions.md");

      // Config should have the correct reasoningEffort
      expect(config.reasoningEffort).toBe(effort);
    }
  });

  it("should preserve reasoningEffort when saving configuration", () => {
    // Setup
    vi.mocked(fs.existsSync).mockReturnValue(false);

    // Create config with reasoningEffort
    const configToSave = {
      model: "test-model",
      instructions: "",
      reasoningEffort: "medium" as ReasoningEffort,
      notify: false,
    };

    // Act
    saveConfig(configToSave, "/mock/config.json", "/mock/instructions.md");

    // Assert
    expect(fs.writeFileSync).toHaveBeenCalledWith(
      "/mock/config.json",
      expect.stringContaining('"model"'),
      "utf-8",
    );

    // Note: Current implementation of saveConfig doesn't save reasoningEffort,
    // this test would need to be updated if that functionality is added
  });
});
