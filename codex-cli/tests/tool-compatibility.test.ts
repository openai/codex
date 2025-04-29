import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  getAvailableTools,
  handleToolCall,
} from "../src/utils/agent/tool-integration";
import { loadPrompts } from "../src/prompts/loader";
import type { ApprovalPolicy } from "../src/approvals";
import type { AppConfig } from "../src/utils/config";

// Mock required dependencies
vi.mock("../src/utils/config", () => ({
  getApiKey: vi.fn().mockReturnValue("test-api-key"),
  getBaseUrl: vi.fn().mockReturnValue("https://api.test.com"),
}));

describe("Tool compatibility across providers", () => {
  let mockConfig: AppConfig;
  let mockApprovalPolicy: ApprovalPolicy;

  beforeEach(() => {
    mockConfig = {
      model: "test-model",
      instructions: "Test instructions",
      provider: "openai",
      mcpEnabled: true, // Enable MCP tools for testing
    } as AppConfig;

    mockApprovalPolicy = "suggest";
  });

  describe("Tool availability", () => {
    it("provides the correct set of tools when browser support is enabled", () => {
      const tools = getAvailableTools(mockConfig, true);

      // Check for core tools
      expect(tools.find((t) => t.name === "shell")).toBeDefined();
      expect(
        tools.find((t) => t.name === "list_code_definition_names"),
      ).toBeDefined();
      expect(
        tools.find((t) => t.name === "ask_followup_question"),
      ).toBeDefined();
      expect(tools.find((t) => t.name === "attempt_completion")).toBeDefined();

      // Check for browser tools
      expect(tools.find((t) => t.name === "browser_action")).toBeDefined();

      // Check for MCP tools
      expect(tools.find((t) => t.name === "use_mcp_tool")).toBeDefined();
      expect(tools.find((t) => t.name === "access_mcp_resource")).toBeDefined();
    });

    it("omits browser tools when browser support is disabled", () => {
      const tools = getAvailableTools(mockConfig, false);

      // Browser tool should be omitted
      expect(tools.find((t) => t.name === "browser_action")).toBeUndefined();

      // Other tools should still be available
      expect(tools.find((t) => t.name === "shell")).toBeDefined();
      expect(
        tools.find((t) => t.name === "list_code_definition_names"),
      ).toBeDefined();
    });

    it("omits MCP tools when MCP is disabled", () => {
      const configWithoutMcp = { ...mockConfig, mcpEnabled: false };
      delete (configWithoutMcp as any).mcpServers;

      const tools = getAvailableTools(configWithoutMcp, true);

      // MCP tools should be omitted
      expect(tools.find((t) => t.name === "use_mcp_tool")).toBeUndefined();
      expect(
        tools.find((t) => t.name === "access_mcp_resource"),
      ).toBeUndefined();

      // Other tools should still be available
      expect(tools.find((t) => t.name === "shell")).toBeDefined();
      expect(
        tools.find((t) => t.name === "list_code_definition_names"),
      ).toBeDefined();
    });
  });

  describe("Tool handling", () => {
    it("handles list_code_definition_names tool correctly", async () => {
      const result = await handleToolCall(
        "list_code_definition_names",
        { path: "/test/path" },
        mockConfig,
        mockApprovalPolicy,
      );

      expect(result.metadata?.success).toBe(true);
      expect(result.output).toContain("/test/path");
    });

    it("handles ask_followup_question tool correctly", async () => {
      const result = await handleToolCall(
        "ask_followup_question",
        {
          question: "What is your preferred language?",
          options: ["JavaScript", "Python", "Go"],
        },
        mockConfig,
        mockApprovalPolicy,
      );

      expect(result.metadata?.success).toBe(true);
      expect(result.output).toContain("What is your preferred language?");
      expect(result.additionalItems?.length).toBeGreaterThan(0);
    });

    it("handles attempt_completion tool correctly", async () => {
      const result = await handleToolCall(
        "attempt_completion",
        {
          result: "Task completed successfully",
          command: "open result.html",
        },
        mockConfig,
        mockApprovalPolicy,
      );

      expect(result.metadata?.success).toBe(true);
      expect(result.output).toContain("Task completion attempted");
      expect(result.additionalItems?.length).toBeGreaterThan(0);

      // Should have the command in the additional items
      expect(
        result.additionalItems?.some((item) =>
          item.content.some((content) =>
            content.text.includes("open result.html"),
          ),
        ),
      ).toBe(true);
    });

    it("handles browser_action tool correctly", async () => {
      const result = await handleToolCall(
        "browser_action",
        {
          action: "launch",
          url: "https://example.com",
        },
        mockConfig,
        mockApprovalPolicy,
      );

      expect(result.metadata?.success).toBe(true);
      expect(result.output).toContain("Browser action requested");
      expect(result.metadata?.["action"]).toBe("launch");
      expect(result.metadata?.["url"]).toBe("https://example.com");
    });

    it("handles use_mcp_tool tool correctly", async () => {
      const result = await handleToolCall(
        "use_mcp_tool",
        {
          server_name: "test-server",
          tool_name: "test-tool",
          arguments: '{"param1": "value1"}',
        },
        mockConfig,
        mockApprovalPolicy,
      );

      expect(result.metadata?.success).toBe(true);
      expect(result.output).toContain("MCP tool call requested");
      expect(result.metadata?.["server_name"]).toBe("test-server");
      expect(result.metadata?.["tool_name"]).toBe("test-tool");
    });

    it("handles access_mcp_resource tool correctly", async () => {
      const result = await handleToolCall(
        "access_mcp_resource",
        {
          server_name: "test-server",
          uri: "test://resource",
        },
        mockConfig,
        mockApprovalPolicy,
      );

      expect(result.metadata?.success).toBe(true);
      expect(result.output).toContain("MCP resource access requested");
      expect(result.metadata?.["server_name"]).toBe("test-server");
      expect(result.metadata?.["uri"]).toBe("test://resource");
    });

    it("handles missing required parameters gracefully", async () => {
      const result = await handleToolCall(
        "ask_followup_question",
        { options: ["Option 1", "Option 2"] }, // Missing required 'question' param
        mockConfig,
        mockApprovalPolicy,
      );

      expect(result.metadata?.success).toBe(false);
      expect(result.output).toContain("Missing required parameter");
    });

    it("handles unknown tools gracefully", async () => {
      const result = await handleToolCall(
        "nonexistent_tool",
        { param: "value" },
        mockConfig,
        mockApprovalPolicy,
      );

      expect(result.metadata?.success).toBe(false);
      expect(result.output).toContain("Unknown tool");
    });
  });

  describe("Tool integration with prompts", () => {
    it("includes tool descriptions in OpenAI system prompt", () => {
      const openaiPrompts = loadPrompts({
        cwd: "/",
        provider: "openai",
        model: "gpt-4-turbo",
      });

      // Verify core tools are mentioned in the system prompt
      expect(openaiPrompts.systemPrompt).toContain(
        "list_code_definition_names",
      );
      expect(openaiPrompts.systemPrompt).toContain("ask_followup_question");
      expect(openaiPrompts.systemPrompt).toContain("attempt_completion");
      expect(openaiPrompts.systemPrompt).toContain("browser_action");
      expect(openaiPrompts.systemPrompt).toContain("use_mcp_tool");
      expect(openaiPrompts.systemPrompt).toContain("access_mcp_resource");
    });

    it("includes tool descriptions in Anthropic system prompt", () => {
      const claudePrompts = loadPrompts({
        cwd: "/",
        provider: "anthropic",
        model: "claude-3-opus",
      });

      // Verify core tools are mentioned in the system prompt
      expect(claudePrompts.systemPrompt).toContain(
        "list_code_definition_names",
      );
      expect(claudePrompts.systemPrompt).toContain("ask_followup_question");
      expect(claudePrompts.systemPrompt).toContain("attempt_completion");
      expect(claudePrompts.systemPrompt).toContain("browser_action");
      expect(claudePrompts.systemPrompt).toContain("use_mcp_tool");
      expect(claudePrompts.systemPrompt).toContain("access_mcp_resource");
    });

    it("includes tool descriptions in Gemini system prompt", () => {
      const geminiPrompts = loadPrompts({
        cwd: "/",
        provider: "gemini",
        model: "gemini-pro",
      });

      // Verify core tools are mentioned in the system prompt
      expect(geminiPrompts.systemPrompt).toContain(
        "list_code_definition_names",
      );
      expect(geminiPrompts.systemPrompt).toContain("ask_followup_question");
      expect(geminiPrompts.systemPrompt).toContain("attempt_completion");
      expect(geminiPrompts.systemPrompt).toContain("browser_action");
      expect(geminiPrompts.systemPrompt).toContain("use_mcp_tool");
      expect(geminiPrompts.systemPrompt).toContain("access_mcp_resource");
    });
  });
});
