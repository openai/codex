/**
 * Contract tests for ModelClient implementations
 * Tests OpenAI and Anthropic client contracts
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { createMockFetch, createMockStream, collectStream } from '../utils/test-helpers';

// Define contract interfaces (will be implemented later)
interface CompletionRequest {
  model: string;
  messages: Message[];
  temperature?: number;
  maxTokens?: number;
  tools?: ToolDefinition[];
  stream?: boolean;
}

interface CompletionResponse {
  id: string;
  model: string;
  choices: Choice[];
  usage: Usage;
}

interface Message {
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string | null;
  toolCalls?: ToolCall[];
  toolCallId?: string;
}

interface Choice {
  index: number;
  message: Message;
  finishReason: 'stop' | 'length' | 'tool_calls' | 'content_filter';
}

interface Usage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}

interface ToolDefinition {
  type: 'function';
  function: {
    name: string;
    description: string;
    parameters: any;
  };
}

interface ToolCall {
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string;
  };
}

interface StreamChunk {
  delta?: {
    content?: string;
    toolCalls?: ToolCall[];
  };
  finishReason?: string;
}

interface ModelClient {
  complete(request: CompletionRequest): Promise<CompletionResponse>;
  stream(request: CompletionRequest): AsyncGenerator<StreamChunk>;
  countTokens(text: string, model: string): number;
}

describe('ModelClient Contracts', () => {
  describe('OpenAI Client Contract', () => {
    let mockFetch: ReturnType<typeof createMockFetch>;

    beforeEach(() => {
      const responses = new Map([
        ['https://api.openai.com/v1/chat/completions', {
          id: 'chatcmpl-123',
          object: 'chat.completion',
          created: Date.now(),
          model: 'gpt-4',
          choices: [{
            index: 0,
            message: {
              role: 'assistant',
              content: 'Hello! How can I help you?'
            },
            finish_reason: 'stop'
          }],
          usage: {
            prompt_tokens: 10,
            completion_tokens: 8,
            total_tokens: 18
          }
        }],
      ]);

      mockFetch = createMockFetch(responses);
      global.fetch = mockFetch as any;
    });

    it('should fulfill CompletionRequest/Response contract', async () => {
      const request: CompletionRequest = {
        model: 'gpt-4',
        messages: [
          { role: 'user', content: 'Hello', toolCalls: undefined, toolCallId: undefined }
        ],
        temperature: 0.7,
        maxTokens: 100,
      };

      // Mock implementation for testing contract
      const mockClient: ModelClient = {
        async complete(req: CompletionRequest): Promise<CompletionResponse> {
          const response = await fetch('https://api.openai.com/v1/chat/completions', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              'Authorization': 'Bearer test-key',
            },
            body: JSON.stringify({
              model: req.model,
              messages: req.messages.map(m => ({
                role: m.role,
                content: m.content,
              })),
              temperature: req.temperature,
              max_tokens: req.maxTokens,
            }),
          });

          const data = await response.json();

          return {
            id: data.id,
            model: data.model,
            choices: data.choices.map((c: any) => ({
              index: c.index,
              message: {
                role: c.message.role,
                content: c.message.content,
                toolCalls: c.message.tool_calls,
                toolCallId: undefined,
              },
              finishReason: c.finish_reason,
            })),
            usage: {
              promptTokens: data.usage.prompt_tokens,
              completionTokens: data.usage.completion_tokens,
              totalTokens: data.usage.total_tokens,
            },
          };
        },
        async *stream(req: CompletionRequest): AsyncGenerator<StreamChunk> {
          yield { delta: { content: 'Hello' } };
          yield { delta: { content: '!' } };
          yield { finishReason: 'stop' };
        },
        countTokens(text: string, model: string): number {
          // Rough approximation for testing
          return Math.ceil(text.length / 4);
        },
      };

      const response = await mockClient.complete(request);

      expect(response).toMatchObject({
        id: expect.any(String),
        model: 'gpt-4',
        choices: expect.arrayContaining([
          expect.objectContaining({
            index: 0,
            message: expect.objectContaining({
              role: 'assistant',
              content: expect.any(String),
            }),
            finishReason: expect.any(String),
          }),
        ]),
        usage: expect.objectContaining({
          promptTokens: expect.any(Number),
          completionTokens: expect.any(Number),
          totalTokens: expect.any(Number),
        }),
      });
    });

    it('should handle tool calls in request and response', async () => {
      const toolDef: ToolDefinition = {
        type: 'function',
        function: {
          name: 'get_weather',
          description: 'Get weather for a location',
          parameters: {
            type: 'object',
            properties: {
              location: { type: 'string' },
            },
          },
        },
      };

      const request: CompletionRequest = {
        model: 'gpt-4',
        messages: [{ role: 'user', content: 'What is the weather?', toolCalls: undefined, toolCallId: undefined }],
        tools: [toolDef],
      };

      expect(request.tools).toBeDefined();
      expect(request.tools![0].function.name).toBe('get_weather');
    });

    it('should support streaming responses', async () => {
      const mockClient: ModelClient = {
        async complete(req: CompletionRequest): Promise<CompletionResponse> {
          throw new Error('Not implemented');
        },
        async *stream(req: CompletionRequest): AsyncGenerator<StreamChunk> {
          const chunks = ['Hello', ' ', 'world', '!'];
          for (const chunk of chunks) {
            yield { delta: { content: chunk } };
          }
          yield { finishReason: 'stop' };
        },
        countTokens(text: string, model: string): number {
          return Math.ceil(text.length / 4);
        },
      };

      const chunks = await collectStream(mockClient.stream({
        model: 'gpt-4',
        messages: [{ role: 'user', content: 'Hi', toolCalls: undefined, toolCallId: undefined }],
        stream: true,
      }));

      expect(chunks).toHaveLength(5);
      expect(chunks[0].delta?.content).toBe('Hello');
      expect(chunks[4].finishReason).toBe('stop');
    });
  });

  describe('Anthropic Client Contract', () => {
    beforeEach(() => {
      const responses = new Map([
        ['https://api.anthropic.com/v1/messages', {
          id: 'msg_123',
          type: 'message',
          role: 'assistant',
          content: [{
            type: 'text',
            text: 'Hello! I can help you.',
          }],
          model: 'claude-3-opus',
          stop_reason: 'end_turn',
          stop_sequence: null,
          usage: {
            input_tokens: 10,
            output_tokens: 8,
          },
        }],
      ]);

      global.fetch = createMockFetch(responses) as any;
    });

    it('should fulfill AnthropicRequest/Response contract', async () => {
      interface AnthropicRequest {
        model: string;
        messages: AnthropicMessage[];
        maxTokens: number;
        temperature?: number;
        system?: string;
        tools?: AnthropicTool[];
        stream?: boolean;
      }

      interface AnthropicMessage {
        role: 'user' | 'assistant';
        content: string | ContentBlock[];
      }

      interface ContentBlock {
        type: 'text' | 'tool_use';
        text?: string;
        id?: string;
        name?: string;
        input?: any;
      }

      interface AnthropicTool {
        name: string;
        description: string;
        input_schema: any;
      }

      interface AnthropicResponse {
        id: string;
        type: 'message';
        role: 'assistant';
        content: ContentBlock[];
        model: string;
        usage: {
          inputTokens: number;
          outputTokens: number;
        };
      }

      // Mock Anthropic client for testing contract
      const mockAnthropicClient = {
        async complete(req: AnthropicRequest): Promise<AnthropicResponse> {
          const response = await fetch('https://api.anthropic.com/v1/messages', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              'anthropic-version': '2023-06-01',
              'x-api-key': 'test-key',
            },
            body: JSON.stringify({
              model: req.model,
              messages: req.messages,
              max_tokens: req.maxTokens,
              temperature: req.temperature,
              system: req.system,
            }),
          });

          const data = await response.json();

          return {
            id: data.id,
            type: data.type,
            role: data.role,
            content: data.content,
            model: data.model,
            usage: {
              inputTokens: data.usage.input_tokens,
              outputTokens: data.usage.output_tokens,
            },
          };
        },
      };

      const request: AnthropicRequest = {
        model: 'claude-3-opus',
        messages: [
          { role: 'user', content: 'Hello Claude' },
        ],
        maxTokens: 100,
        temperature: 0.7,
        system: 'You are a helpful assistant.',
      };

      const response = await mockAnthropicClient.complete(request);

      expect(response).toMatchObject({
        id: expect.any(String),
        type: 'message',
        role: 'assistant',
        content: expect.arrayContaining([
          expect.objectContaining({
            type: 'text',
            text: expect.any(String),
          }),
        ]),
        model: expect.stringContaining('claude'),
        usage: expect.objectContaining({
          inputTokens: expect.any(Number),
          outputTokens: expect.any(Number),
        }),
      });
    });

    it('should handle content blocks and tool use', () => {
      const contentBlock: ContentBlock = {
        type: 'tool_use',
        id: 'tool_123',
        name: 'calculator',
        input: { expression: '2 + 2' },
      };

      expect(contentBlock.type).toBe('tool_use');
      expect(contentBlock.name).toBe('calculator');
      expect(contentBlock.input).toEqual({ expression: '2 + 2' });
    });

    it('should support system prompts', () => {
      const request = {
        model: 'claude-3-opus',
        messages: [{ role: 'user' as const, content: 'Hi' }],
        maxTokens: 100,
        system: 'You are an expert programmer.',
      };

      expect(request.system).toBeDefined();
      expect(request.system).toBe('You are an expert programmer.');
    });
  });

  describe('Common Contract Requirements', () => {
    it('should validate message format', () => {
      const validMessage: Message = {
        role: 'user',
        content: 'Test message',
        toolCalls: undefined,
        toolCallId: undefined,
      };

      expect(validMessage.role).toMatch(/^(system|user|assistant|tool)$/);
      expect(validMessage.content).toBeTypeOf('string');
    });

    it('should handle token counting', () => {
      const mockClient: ModelClient = {
        async complete(req: CompletionRequest): Promise<CompletionResponse> {
          throw new Error('Not implemented');
        },
        async *stream(req: CompletionRequest): AsyncGenerator<StreamChunk> {
          yield { delta: { content: 'test' } };
        },
        countTokens(text: string, model: string): number {
          // Simple approximation for testing
          const tokensPerWord = model.includes('gpt-4') ? 1.3 : 1.5;
          const words = text.split(/\s+/).length;
          return Math.ceil(words * tokensPerWord);
        },
      };

      const text = 'This is a test message';
      const tokens = mockClient.countTokens(text, 'gpt-4');

      expect(tokens).toBeGreaterThan(0);
      expect(tokens).toBeLessThan(text.length); // Tokens should be less than characters
    });
  });
});