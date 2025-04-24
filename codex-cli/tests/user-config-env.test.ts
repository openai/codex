import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { mkdtempSync, writeFileSync, rmSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";

/**
 * Verifies that ~/.codex.env is parsed (lowest‑priority) when present.
 */

describe("user‑wide ~/.codex.env support", () => {
  const ORIGINAL_HOME = process.env["HOME"];
  const ORIGINAL_API_KEY = process.env["OPENAI_API_KEY"];

  let tempHome: string;

  beforeEach(() => {
    // Create an isolated fake $HOME directory.
    tempHome = mkdtempSync(join(tmpdir(), "codex-home-"));
    process.env["HOME"] = tempHome;

    // Ensure the env var is unset so that the file value is picked up.
    delete process.env["OPENAI_API_KEY"];

    // Write ~/.codex.env with a dummy key.
    writeFileSync(
      join(tempHome, ".codex.env"),
      "OPENAI_API_KEY=my-home-key\n",
      {
        encoding: "utf8",
      },
    );
  });

  afterEach(() => {
    // Cleanup temp directory.
    try {
      rmSync(tempHome, { recursive: true, force: true });
    } catch {
      // ignore
    }

    // Restore original env.
    if (ORIGINAL_HOME !== undefined) {
      process.env["HOME"] = ORIGINAL_HOME;
    } else {
      delete process.env["HOME"];
    }

    if (ORIGINAL_API_KEY !== undefined) {
      process.env["OPENAI_API_KEY"] = ORIGINAL_API_KEY;
    } else {
      delete process.env["OPENAI_API_KEY"];
    }
  });

  it("loads the API key from ~/.codex.env when not set elsewhere", async () => {
    // Import the config module AFTER setting up the fake env.
    const { getApiKey } = await import("../src/utils/config.js");

    expect(getApiKey("openai")).toBe("my-home-key");
  });
});
