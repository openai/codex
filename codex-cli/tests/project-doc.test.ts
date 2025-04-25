import { loadConfig, PROJECT_DOC_MAX_BYTES } from "../src/utils/config.js";
import { mkdirSync, rmSync, writeFileSync, mkdtempSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { describe, expect, test, beforeEach, afterEach, vi } from "vitest";

let projectDir: string;
let configPath: string;
let instructionsPath: string;

beforeEach(() => {
  projectDir = mkdtempSync(join(tmpdir(), "codex-proj-"));
  // Create fake .git dir to mark project root
  mkdirSync(join(projectDir, ".git"));

  // Config & instructions paths under temp dir so we don't pollute real homedir
  configPath = join(projectDir, "config.json");
  instructionsPath = join(projectDir, "instructions.md");
});

afterEach(() => {
  rmSync(projectDir, { recursive: true, force: true });
});

describe("project doc integration", () => {
  test("happy path: project doc gets merged into instructions", () => {
    const docContent = "# Project\nThis is my project.";
    writeFileSync(join(projectDir, "codex.md"), docContent);

    const cfg = loadConfig(configPath, instructionsPath, { cwd: projectDir });
    expect(cfg.instructions).toContain(docContent);
  });

  test("opt-out via flag prevents inclusion", () => {
    const docContent = "will be ignored";
    writeFileSync(join(projectDir, "codex.md"), docContent);

    const cfg = loadConfig(configPath, instructionsPath, {
      cwd: projectDir,
      disableProjectDoc: true,
    });
    expect(cfg.instructions).not.toContain(docContent);
  });

  test("file larger than limit gets truncated and warns", () => {
    const big = "x".repeat(PROJECT_DOC_MAX_BYTES + 4096);
    writeFileSync(join(projectDir, "codex.md"), big);

    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const cfg = loadConfig(configPath, instructionsPath, { cwd: projectDir });

    expect(cfg.instructions.length).toBe(PROJECT_DOC_MAX_BYTES);
    expect(warnSpy).toHaveBeenCalledOnce();

    warnSpy.mockRestore();
  });
});
