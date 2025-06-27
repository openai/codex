import { describe, it, expect, vi, afterEach } from "vitest";

// The model‑utils module reads OPENAI_API_KEY at import time. We therefore
// need to tweak the env var *before* importing the module in each test and
// make sure the module cache is cleared.

const ORIGINAL_ENV_KEY = process.env["OPENAI_API_KEY"];

// Holders so individual tests can adjust behaviour of the OpenAI mock.
const openAiState: { listSpy?: ReturnType<typeof vi.fn> } = {};

vi.mock("openai", () => {
  class FakeOpenAI {
    public models = {
      // `listSpy` will be swapped out by the tests
      list: (...args: Array<any>) => openAiState.listSpy!(...args),
    };
  }

  return {
    __esModule: true,
    default: FakeOpenAI,
  };
});

describe("model-utils – offline resilience", () => {
  afterEach(() => {
    // Restore env var & module cache so tests are isolated.
    if (ORIGINAL_ENV_KEY !== undefined) {
      process.env["OPENAI_API_KEY"] = ORIGINAL_ENV_KEY;
    } else {
      delete process.env["OPENAI_API_KEY"];
    }
    vi.resetModules();
    openAiState.listSpy = undefined;
  });

  it("returns true when API key absent (no network available)", async () => {
    delete process.env["OPENAI_API_KEY"];

    // Re‑import after env change so the module picks up the new state.
    vi.resetModules();
    const { isModelSupportedForResponses } = await import(
      "../src/utils/model-utils.js"
    );

    const supported = await isModelSupportedForResponses(
      "openai",
      "codex-mini-latest",
    );
    expect(supported).toBe(true);
  });

  it("falls back gracefully when openai.models.list throws a network error", async () => {
    process.env["OPENAI_API_KEY"] = "dummy";

    const netErr: any = new Error("socket hang up");
    netErr.code = "ECONNRESET";

    openAiState.listSpy = vi.fn(async () => {
      throw netErr;
    });

    vi.resetModules();
    const { isModelSupportedForResponses } = await import(
      "../src/utils/model-utils.js"
    );

    // Should resolve true despite the network failure.
    const supported = await isModelSupportedForResponses(
      "openai",
      "some-model",
    );
    expect(supported).toBe(true);
  });
});
