import { describe, it, expect, beforeEach, afterEach } from "vitest";

// We import the module *lazily* inside each test so that we can control the
// OPENROUTER_API_KEY env var independently per test case. Node's module cache
// would otherwise capture the value present during the first import.

const ORIGINAL_ENV_KEY = process.env["OPENROUTER_API_KEY"];

beforeEach(() => {
  delete process.env["OPENROUTER_API_KEY"];
});

afterEach(() => {
  if (ORIGINAL_ENV_KEY !== undefined) {
    process.env["OPENROUTER_API_KEY"] = ORIGINAL_ENV_KEY;
  } else {
    delete process.env["OPENROUTER_API_KEY"];
  }
});

describe("config.setApiKey", () => {
  it("overrides the exported OPENROUTER_API_KEY at runtime", async () => {
    const { setApiKey, OPENROUTER_API_KEY: OPENROUTER_API_KEY } = await import(
      "../src/utils/config.js"
    );

    expect(OPENROUTER_API_KEY).toBe("");

    setApiKey("my‑key");

    const { OPENROUTER_API_KEY: liveRef } = await import("../src/utils/config.js");

    expect(liveRef).toBe("my‑key");
  });
});
