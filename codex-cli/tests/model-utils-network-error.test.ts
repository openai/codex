import { describe, it, expect, vi, afterEach } from "vitest";

// The model‑utils module reads provider API keys at import time. We therefore
// need to tweak the env vars *before* importing the module in each test and
// make sure the module cache is cleared.

const ORIGINAL_ENV_KEY = process.env["OPENAI_API_KEY"];

// Holders so individual tests can adjust behaviour of the OpenAI mock.
const openAiState: { listSpy?: ReturnType<typeof vi.fn> } = {};

vi.mock("openai", () => {
  class FakeOpenAI {
    constructor() {
      // Ensure the constructor doesn't throw
    }
    
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

// Mock provider-config module
vi.mock("../src/utils/provider-config.js", () => {
  return {
    DEFAULT_PROVIDER_ID: "openai",
    DEFAULT_PROVIDER_MODELS: {
      openai: "o4-mini",
      claude: "claude-3-sonnet-20240229"
    }
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

    const supported = await isModelSupportedForResponses("o4-mini");
    expect(supported).toBe(true);
  });

  it.skip("falls back gracefully when openai.models.list throws a network error", async () => {
    process.env["OPENAI_API_KEY"] = "dummy";

    const netErr: any = new Error("socket hang up");
    netErr.code = "ECONNRESET";

    // Make the spy always throw an error
    openAiState.listSpy = vi.fn(async () => {
      throw netErr;
    });

    vi.resetModules();
    
    // Set up providerApiKeys mock for the test
    const configModule = await import("../src/utils/config.js");
    configModule.providerApiKeys.openai = "dummy"; 
    
    const modelUtils = await import("../src/utils/model-utils.js");
    
    // Make sure we reset the cache before testing
    modelUtils.resetModelsCache();
    
    // With our implementation, this should now return true
    const supported = await modelUtils.isModelSupportedForResponses("some-model");
    expect(supported).toBe(true);
  });
});
