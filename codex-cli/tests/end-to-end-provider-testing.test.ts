import { describe, it, expect, vi } from "vitest";
import { getAvailableTools } from "../src/utils/agent/tool-integration";
import type { AppConfig } from "../src/utils/config";

// Mock required dependencies
vi.mock("../src/utils/config", () => ({
  getApiKey: vi.fn().mockReturnValue("test-api-key"),
  getBaseUrl: vi.fn().mockReturnValue("https://api.test.com"),
  OPENAI_TIMEOUT_MS: 60000,
  OPENAI_ORGANIZATION: "test-org",
  OPENAI_PROJECT: "test-project",
}));

// Mock the agent loop module to prevent actual API calls
vi.mock("../src/utils/agent/agent-loop", () => ({
  AgentLoop: vi.fn().mockImplementation(() => ({
    run: vi.fn().mockResolvedValue(undefined),
    cancel: vi.fn(),
    terminate: vi.fn(),
  })),
}));

// Skip real requests during testing - we just need to check the structures
vi.mock("../src/utils/responses", () => ({
  responsesCreateViaChatCompletions: vi.fn(),
}));

// Mock API providers
vi.mock("openai", () => ({
  default: vi.fn().mockImplementation(() => ({
    chat: { completions: { create: vi.fn() } },
    responses: { create: vi.fn() },
  })),
  APIConnectionTimeoutError: class APIConnectionTimeoutError extends Error {},
  APIConnectionError: class APIConnectionError extends Error {},
}));

vi.mock("@anthropic-ai/sdk", () => ({
  default: vi.fn().mockImplementation(() => ({
    messages: { create: vi.fn() },
  })),
}));

vi.mock("@google/generative-ai", () => ({
  GoogleGenerativeAI: vi.fn().mockImplementation(() => ({
    getGenerativeModel: vi.fn().mockReturnValue({
      generateContent: vi.fn(),
    }),
  })),
}));

describe("End-to-end provider testing", () => {
  describe("Tool integration", () => {
    it("correctly includes all required tools across providers", () => {
      // Create a mock config for testing
      const mockConfig: AppConfig = {
        model: "test-model",
        instructions: "Test instructions",
        provider: "openai",
        mcpEnabled: true,
      } as AppConfig;

      // Check that all needed tools are available
      const tools = getAvailableTools(mockConfig);

      // Check for core tools that should be available
      expect(
        tools.find((t) => t.name === "list_code_definition_names"),
      ).toBeDefined();
      expect(
        tools.find((t) => t.name === "ask_followup_question"),
      ).toBeDefined();
      expect(tools.find((t) => t.name === "attempt_completion")).toBeDefined();
      expect(tools.find((t) => t.name === "browser_action")).toBeDefined();

      // Check for MCP tools
      expect(tools.find((t) => t.name === "use_mcp_tool")).toBeDefined();
      expect(tools.find((t) => t.name === "access_mcp_resource")).toBeDefined();

      // Check for presence of specific tools based on the tool-integration.ts implementation

      // Core shell tool
      expect(tools.find((t) => t.name === "shell")).toBeDefined();

      // File operations (through execute_command in actual usage)
      expect(tools.length).toBeGreaterThan(1);
    });
  });
});
