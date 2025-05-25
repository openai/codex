/**
 * codex/codex-cli/tests/disableResponseStorage.test.ts
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { mkdtempSync, rmSync, writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { loadConfig, saveConfig } from "../src/utils/config";
import type { AppConfig } from "../src/utils/config";

const sandboxHome: string = mkdtempSync(join(tmpdir(), "codex-home-"));
const codexDir: string = join(sandboxHome, ".codex");
const yamlPath: string = join(codexDir, "config.yaml");

describe("disableResponseStorage persistence", () => {
  beforeAll((): void => {
    // mkdir -p ~/.codex inside the sandbox
    rmSync(codexDir, { recursive: true, force: true });
    mkdirSync(codexDir, { recursive: true });

    // seed YAML with ZDR enabled
    writeFileSync(
      yamlPath,
      "model: codex-mini-latest\ndisableResponseStorage: true\n",
    );
  });

  afterAll((): void => {
    rmSync(sandboxHome, { recursive: true, force: true });
  });

  it("keeps disableResponseStorage=true across load/save cycle", async (): Promise<void> => {
    // 1️⃣ explicitly load the sandbox file
    const cfg1: AppConfig = loadConfig(yamlPath);
    expect(cfg1.disableResponseStorage).toBe(true);

    // 2️⃣ save right back to the same file
    await saveConfig(cfg1, yamlPath);

    // 3️⃣ reload and re-assert
    const cfg2: AppConfig = loadConfig(yamlPath);
    expect(cfg2.disableResponseStorage).toBe(true);
  });
});
