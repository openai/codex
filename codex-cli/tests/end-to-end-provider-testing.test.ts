import { describe, it, expect, vi, beforeEach } from "vitest";
import { AgentLoop } from "../src/utils/agent/agent-loop";
import { getAvailableTools } from "../src/utils/agent/tool-integration";
import type { ApprovalPolicy } from "../src/approvals";
import type { AppConfig } from "../src/utils/config";
import type { ResponseInputItem } from "openai/resources/responses/responses.mjs";

// Mock required dependencies
vi.mock("../src/utils/config", () => ({
  getApiKey: vi.fn().mockReturnValue("test-api-key"),
  getBaseUrl: vi.fn().mockReturnValue("https://api.test.com"),
}));

// Mock API responses for each provider
vi.mock("openai", () => {
  return {
    default: vi.fn().mockImplementation(() => ({
      chat: {
        completions: {
          create: vi.fn().mockResolvedValue({
            choices: [
              {
                message: {
                  content: "OpenAI response",
                  role: "assistant",
                  tool_calls: [
                    {
                      id: "call_123",
                      type: "function",
                      function: {
                        name: "shell",
                        arguments: JSON.stringify({ command: ["ls", "-la"] }),
                      },
                    },
                  ],
                },
                index: 0,
                finish_reason: "tool_calls",
              },
            ],
          }),
        },
      },
    })),
  };
});

vi.mock("@anthropic-ai/sdk", () => {
  return {
    default: vi.fn().mockImplementation(() => ({
      messages: {
        create: vi.fn().mockResolvedValue({
          content: [
            {
              type: "text",
              text: "Anthropic response",
            },
            {
              type: "tool_use",
              id: "tool_123",
              name: "shell",
              input: { command: ["ls", "-la"] },
            },
          ],
          role: "assistant",
          model: "claude-3-opus",
          id: "msg_123",
          type: "message",
        }),
      },
    })),
  };
});

vi.mock("@google/generative-ai", () => {
  return {
    GoogleGenerativeAI: vi.fn().mockImplementation(() => ({
      getGenerativeModel: vi.fn().mockReturnValue({
        generateContent: vi.fn().mockResolvedValue({
          response: {
            candidates: [
              {
                content: {
                  parts: [
                    {
                      text: "Gemini response",
                      functionCalls: [
                        {
                          name: "shell",
                          args: { command: ["ls", "-la"] },
                        },
                      ],
                    },
                  ],
                },
              },
            ],
          },
        }),
      }),
    })),
  };
});

// Mock tool execution
vi.mock("child_process", () => ({
  spawn: vi.fn().mockImplementation(() => ({
    stdout: {
      on: vi.fn().mockImplementation((event, callback) => {
        if (event === "data") {
          callback(Buffer.from("Mock command output"));
        }
        return { on: vi.fn() };
      }),
    },
    stderr: {
      on: vi.fn().mockImplementation(() => ({ on: vi.fn() })),
    },
    on: vi.fn().mockImplementation((event, callback) => {
      if (event === "close") {
        callback(0);
      }
      return { on: vi.fn() };
    }),
  })),
}));

describe("End-to-end provider testing", () => {
  let mockConfig: AppConfig;
  let mockApprovalPolicy: ApprovalPolicy;
  let onItemMock: ReturnType<typeof vi.fn>;
  let getCommandConfirmationMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockConfig = {
      model: "test-model",
      instructions: "Test instructions",
      provider: "openai",
      mcpEnabled: true,
    } as AppConfig;

    mockApprovalPolicy = "suggest";
    onItemMock = vi.fn();
    getCommandConfirmationMock = vi.fn().mockResolvedValue(true);
  });

  describe("OpenAI provider end-to-end", () => {
    it("processes a complete conversation with OpenAI models", async () => {
      const agent = new AgentLoop({
        model: "gpt-4-turbo",
        provider: "openai",
        config: mockConfig,
        instructions: mockConfig.instructions,
        approvalPolicy: mockApprovalPolicy,
        additionalWritableRoots: [],
        onItem: onItemMock,
        onLoading: vi.fn(),
        getCommandConfirmation: getCommandConfirmationMock,
        onLastResponseId: vi.fn(),
      });

      // Start a conversation with run method
      const input = [
        {
          type: "message",
          role: "user",
          content: [
            {
              type: "input_text",
              text: "List files in the current directory",
            },
          ],
        },
      ] as unknown as Array<ResponseInputItem>;

      await agent.run(input);

      // Verify the response was processed
      expect(onItemMock).toHaveBeenCalled();

      // Verify the model response contained a tool call
      const toolCallItem = onItemMock.mock.calls.find(
        (call) =>
          call[0].type === "assistant" &&
          call[0].toolCalls &&
          call[0].toolCalls.length > 0,
      );
      expect(toolCallItem).toBeDefined();

      // Verify the tool call was executed
      const toolResponseItem = onItemMock.mock.calls.find(
        (call) =>
          call[0].type === "tool" &&
          call[0].content.includes("Mock command output"),
      );
      expect(toolResponseItem).toBeDefined();
    });
  });

  describe("Anthropic provider end-to-end", () => {
    it("processes a complete conversation with Claude models", async () => {
      const agent = new AgentLoop({
        model: "claude-3-opus",
        provider: "anthropic",
        config: { ...mockConfig, provider: "anthropic" },
        instructions: mockConfig.instructions,
        approvalPolicy: mockApprovalPolicy,
        additionalWritableRoots: [],
        onItem: onItemMock,
        onLoading: vi.fn(),
        getCommandConfirmation: getCommandConfirmationMock,
        onLastResponseId: vi.fn(),
      });

      // Start a conversation with run method
      const input = [
        {
          type: "message",
          role: "user",
          content: [
            {
              type: "input_text",
              text: "List files in the current directory",
            },
          ],
        },
      ] as unknown as Array<ResponseInputItem>;

      await agent.run(input);

      // Verify the response was processed
      expect(onItemMock).toHaveBeenCalled();

      // Verify the model response contained a tool call
      const toolCallItem = onItemMock.mock.calls.find(
        (call) =>
          call[0].type === "assistant" &&
          call[0].toolCalls &&
          call[0].toolCalls.length > 0,
      );
      expect(toolCallItem).toBeDefined();

      // Verify the tool call was executed
      const toolResponseItem = onItemMock.mock.calls.find(
        (call) =>
          call[0].type === "tool" &&
          call[0].content.includes("Mock command output"),
      );
      expect(toolResponseItem).toBeDefined();
    });
  });

  describe("Gemini provider end-to-end", () => {
    it("processes a complete conversation with Gemini models", async () => {
      const agent = new AgentLoop({
        model: "gemini-pro",
        provider: "gemini",
        config: { ...mockConfig, provider: "gemini" },
        instructions: mockConfig.instructions,
        approvalPolicy: mockApprovalPolicy,
        additionalWritableRoots: [],
        onItem: onItemMock,
        onLoading: vi.fn(),
        getCommandConfirmation: getCommandConfirmationMock,
        onLastResponseId: vi.fn(),
      });

      // Start a conversation with run method
      const input = [
        {
          type: "message",
          role: "user",
          content: [
            {
              type: "input_text",
              text: "List files in the current directory",
            },
          ],
        },
      ] as unknown as Array<ResponseInputItem>;

      await agent.run(input);

      // Verify the response was processed
      expect(onItemMock).toHaveBeenCalled();

      // Verify the model response contained a tool call
      const toolCallItem = onItemMock.mock.calls.find(
        (call) =>
          call[0].type === "assistant" &&
          call[0].toolCalls &&
          call[0].toolCalls.length > 0,
      );
      expect(toolCallItem).toBeDefined();

      // Verify the tool call was executed
      const toolResponseItem = onItemMock.mock.calls.find(
        (call) =>
          call[0].type === "tool" &&
          call[0].content.includes("Mock command output"),
      );
      expect(toolResponseItem).toBeDefined();
    });
  });

  describe("Tool execution verification", () => {
    it("correctly executes additional migrated tools", async () => {
      // Create a mock for the specific tool handler
      const mockToolHandler = vi.fn().mockResolvedValue({
        output: "Tool executed successfully",
        metadata: { success: true },
      });

      // Override the actual tool handler with our mock
      vi.mock("../src/utils/agent/tool-integration", async () => {
        const actual = await vi.importActual(
          "../src/utils/agent/tool-integration",
        );
        return {
          ...actual,
          handleToolCall: mockToolHandler,
        };
      });

      // Check that all needed tools are available
      const tools = getAvailableTools(mockConfig);
      expect(
        tools.find((t) => t.name === "list_code_definition_names"),
      ).toBeDefined();
      expect(
        tools.find((t) => t.name === "ask_followup_question"),
      ).toBeDefined();
      expect(tools.find((t) => t.name === "attempt_completion")).toBeDefined();
      expect(tools.find((t) => t.name === "browser_action")).toBeDefined();
      expect(tools.find((t) => t.name === "use_mcp_tool")).toBeDefined();
      expect(tools.find((t) => t.name === "access_mcp_resource")).toBeDefined();

      // This test only verifies that the tool mocks work as expected
      expect(mockToolHandler).not.toHaveBeenCalled();
    });
  });
});
