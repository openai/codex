import { describe, it, expect, vi, afterEach } from "vitest";

// The model‑utils module reads OPENAI_API_KEY at import time. We therefore
// need to tweak the env var *before* importing the module in each test and
// make sure the module cache is cleared.

const ORIGINAL_ENV_KEY = process.env["OPENAI_API_KEY"];
const ORIGINAL_AZURE_ENDPOINT = process.env["AZURE_OPENAI_ENDPOINT"];

// Holders so individual tests can adjust behaviour of the OpenAI mock.
const openAiState: { listSpy?: ReturnType<typeof vi.fn> } = {};

vi.mock("openai", () => {
  class FakeOpenAI {
    public models = {
      // `listSpy` will be swapped out by the tests
      list: (...args: Array<any>) => openAiState.listSpy!(...args),
    };
  }

  class FakeAzureOpenAI {
    public models = {
      list: (...args: Array<any>) => openAiState.listSpy!(...args),
    };
  }

  return {
    __esModule: true,
    default: FakeOpenAI,
    AzureOpenAI: FakeAzureOpenAI,
  };
});

// Mock Azure Identity
vi.mock("@azure/identity", () => {
  return {
    DefaultAzureCredential: class {},
    getBearerTokenProvider: () => ({}),
  };
});

describe("model-utils – offline resilience", () => {
  afterEach(() => {
    // Restore env vars & module cache so tests are isolated.
    if (ORIGINAL_ENV_KEY !== undefined) {
      process.env["OPENAI_API_KEY"] = ORIGINAL_ENV_KEY;
    } else {
      delete process.env["OPENAI_API_KEY"];
    }
    if (ORIGINAL_AZURE_ENDPOINT !== undefined) {
      process.env["AZURE_OPENAI_ENDPOINT"] = ORIGINAL_AZURE_ENDPOINT;
    } else {
      delete process.env["AZURE_OPENAI_ENDPOINT"];
    }
    vi.resetModules();
    openAiState.listSpy = undefined;
  });

  it("returns true when API key absent (no network available)", async () => {
    delete process.env["OPENAI_API_KEY"];
    delete process.env["AZURE_OPENAI_ENDPOINT"];

    // Re‑import after env change so the module picks up the new state.
    vi.resetModules();
    const { isModelSupportedForResponses } = await import(
      "../src/utils/model-utils.js"
    );

    const supported = await isModelSupportedForResponses("o4-mini");
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
    const { isModelSupportedForResponses, RECOMMENDED_MODELS } = await import(
      "../src/utils/model-utils.js"
    );

    // Should resolve true despite the network failure
    const supported = await isModelSupportedForResponses(RECOMMENDED_MODELS[0]);
    expect(supported).toBe(true);
  });
});
