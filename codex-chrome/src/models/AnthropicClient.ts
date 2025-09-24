/**
 * Anthropic Claude API client implementation for codex-chrome
 * Implements the ModelClient interface with Anthropic-specific functionality
 */

import {
  ModelClient,
  ModelClientError,
  type CompletionRequest,
  type CompletionResponse,
  type StreamChunk,
  type Message,
  type ToolCall,
} from './ModelClient';

/**
 * Anthropic-specific request format
 */
interface AnthropicCompletionRequest {
  model: string;
  messages: AnthropicMessage[];
  max_tokens: number;
  temperature?: number;
  system?: string;
  tools?: AnthropicTool[];
  stream?: boolean;
}

/**
 * Anthropic message format
 */
interface AnthropicMessage {
  role: 'user' | 'assistant';
  content: string | ContentBlock[];
}

/**
 * Anthropic content block types
 */
interface ContentBlock {
  type: 'text' | 'tool_use' | 'tool_result';
  text?: string;
  id?: string;
  name?: string;
  input?: any;
  tool_use_id?: string;
  content?: string;
}

/**
 * Anthropic tool definition format
 */
interface AnthropicTool {
  name: string;
  description: string;
  input_schema: any;
}

/**
 * Anthropic completion response format
 */
interface AnthropicCompletionResponse {
  id: string;
  type: 'message';
  role: 'assistant';
  content: ContentBlock[];
  model: string;
  stop_reason: 'end_turn' | 'max_tokens' | 'stop_sequence' | 'tool_use';
  stop_sequence?: string;
  usage: {
    input_tokens: number;
    output_tokens: number;
  };
}

/**
 * Anthropic streaming chunk format
 */
interface AnthropicStreamChunk {
  type: 'message_start' | 'message_delta' | 'content_block_start' | 'content_block_delta' | 'content_block_stop' | 'message_stop';
  message?: {
    id: string;
    type: 'message';
    role: 'assistant';
    content: ContentBlock[];
    model: string;
    usage?: {
      input_tokens: number;
      output_tokens: number;
    };
  };
  delta?: {
    text?: string;
    stop_reason?: string;
    usage?: {
      output_tokens: number;
    };
  };
  content_block?: ContentBlock;
  index?: number;
}

/**
 * Token counting mappings for different Claude models
 */
const CLAUDE_TOKEN_MULTIPLIERS: Record<string, number> = {
  'claude-3-opus-20240229': 1.2,
  'claude-3-sonnet-20240229': 1.2,
  'claude-3-haiku-20240307': 1.2,
  'claude-3-5-sonnet-20240620': 1.2,
  'claude-3-5-haiku-20241022': 1.2,
};

/**
 * Anthropic Claude API client implementation
 */
export class AnthropicClient extends ModelClient {
  private readonly apiKey: string;
  private readonly baseUrl: string;
  private readonly version: string;

  constructor(
    apiKey: string,
    options: {
      baseUrl?: string;
      version?: string;
    } = {}
  ) {
    super();

    if (!apiKey?.trim()) {
      throw new ModelClientError('Anthropic API key is required');
    }

    this.apiKey = apiKey;
    this.baseUrl = options.baseUrl || 'https://api.anthropic.com';
    this.version = options.version || '2023-06-01';
  }

  getProvider(): string {
    return 'anthropic';
  }

  async complete(request: CompletionRequest): Promise<CompletionResponse> {
    this.validateRequest(request);

    const anthropicRequest = this.convertToAnthropicRequest(request);

    const response = await this.withRetry(
      () => this.makeRequest(anthropicRequest),
      (error) => this.isRetryableError(error)
    );

    return this.convertFromAnthropicResponse(response);
  }

  async *stream(request: CompletionRequest): AsyncGenerator<StreamChunk> {
    this.validateRequest(request);

    const anthropicRequest = this.convertToAnthropicRequest({ ...request, stream: true });

    const response = await this.withRetry(
      () => this.makeStreamRequest(anthropicRequest),
      (error) => this.isRetryableError(error)
    );

    if (!response.body) {
      throw new ModelClientError('Stream response body is null');
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';

    try {
      while (true) {
        const { done, value } = await reader.read();

        if (done) {
          break;
        }

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split('\n');

        // Keep the last potentially incomplete line in the buffer
        buffer = lines.pop() || '';

        for (const line of lines) {
          const trimmed = line.trim();

          if (!trimmed || !trimmed.startsWith('data: ')) {
            continue;
          }

          const data = trimmed.slice(6); // Remove 'data: ' prefix

          try {
            const chunk: AnthropicStreamChunk = JSON.parse(data);
            const streamChunk = this.convertStreamChunk(chunk);

            if (streamChunk) {
              yield streamChunk;
            }
          } catch (error) {
            // Skip malformed chunks
            console.warn('Failed to parse stream chunk:', error);
          }
        }
      }
    } finally {
      reader.releaseLock();
    }
  }

  countTokens(text: string, model: string): number {
    // Simple token counting approximation for Claude
    // Claude uses a similar tokenization approach to GPT models
    const multiplier = CLAUDE_TOKEN_MULTIPLIERS[model] || 1.2;
    const words = text.split(/\s+/).length;
    const punctuation = (text.match(/[.!?;:,]/g) || []).length;

    return Math.ceil((words + punctuation * 0.5) * multiplier);
  }

  private convertToAnthropicRequest(request: CompletionRequest): AnthropicCompletionRequest {
    const { systemMessage, userMessages } = this.extractSystemMessage(request.messages);

    return {
      model: request.model,
      messages: userMessages.map(this.convertMessage),
      max_tokens: request.maxTokens || 4096, // Claude requires max_tokens
      temperature: request.temperature,
      system: systemMessage,
      tools: request.tools?.map(tool => ({
        name: tool.function.name,
        description: tool.function.description,
        input_schema: tool.function.parameters,
      })),
      stream: request.stream,
    };
  }

  private extractSystemMessage(messages: Message[]): { systemMessage?: string; userMessages: Message[] } {
    const systemMessages = messages.filter(msg => msg.role === 'system');
    const userMessages = messages.filter(msg => msg.role !== 'system');

    const systemMessage = systemMessages.length > 0
      ? systemMessages.map(msg => msg.content).filter(Boolean).join('\n\n')
      : undefined;

    return { systemMessage, userMessages };
  }

  private convertMessage(message: Message): AnthropicMessage {
    if (message.role === 'tool') {
      // Convert tool response to content block format
      return {
        role: 'user',
        content: [{
          type: 'tool_result',
          tool_use_id: message.toolCallId!,
          content: message.content || '',
        }],
      };
    }

    if (message.toolCalls && message.toolCalls.length > 0) {
      // Convert tool calls to content blocks
      const content: ContentBlock[] = [];

      // Add text content if present
      if (message.content) {
        content.push({
          type: 'text',
          text: message.content,
        });
      }

      // Add tool use blocks
      for (const toolCall of message.toolCalls) {
        content.push({
          type: 'tool_use',
          id: toolCall.id,
          name: toolCall.function.name,
          input: JSON.parse(toolCall.function.arguments),
        });
      }

      return {
        role: 'assistant',
        content,
      };
    }

    // Simple text message
    return {
      role: message.role as 'user' | 'assistant',
      content: message.content || '',
    };
  }

  private convertFromAnthropicResponse(response: AnthropicCompletionResponse): CompletionResponse {
    // Extract text content and tool calls from content blocks
    let textContent = '';
    const toolCalls: ToolCall[] = [];

    for (const block of response.content) {
      if (block.type === 'text' && block.text) {
        textContent += block.text;
      } else if (block.type === 'tool_use' && block.id && block.name) {
        toolCalls.push({
          id: block.id,
          type: 'function',
          function: {
            name: block.name,
            arguments: JSON.stringify(block.input || {}),
          },
        });
      }
    }

    const message: Message = {
      role: 'assistant',
      content: textContent || null,
      toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
    };

    return {
      id: response.id,
      model: response.model,
      choices: [{
        index: 0,
        message,
        finishReason: this.convertStopReason(response.stop_reason),
      }],
      usage: {
        promptTokens: response.usage.input_tokens,
        completionTokens: response.usage.output_tokens,
        totalTokens: response.usage.input_tokens + response.usage.output_tokens,
      },
    };
  }

  private convertStopReason(stopReason: string): 'stop' | 'length' | 'tool_calls' | 'content_filter' {
    switch (stopReason) {
      case 'end_turn':
        return 'stop';
      case 'max_tokens':
        return 'length';
      case 'tool_use':
        return 'tool_calls';
      default:
        return 'stop';
    }
  }

  private convertStreamChunk(chunk: AnthropicStreamChunk): StreamChunk | null {
    switch (chunk.type) {
      case 'content_block_delta':
        if (chunk.delta?.text) {
          return {
            delta: {
              content: chunk.delta.text,
            },
          };
        }
        break;

      case 'content_block_start':
        if (chunk.content_block?.type === 'tool_use') {
          const toolCall: ToolCall = {
            id: chunk.content_block.id!,
            type: 'function',
            function: {
              name: chunk.content_block.name!,
              arguments: JSON.stringify(chunk.content_block.input || {}),
            },
          };

          return {
            delta: {
              toolCalls: [toolCall],
            },
          };
        }
        break;

      case 'message_stop':
        return {
          finishReason: 'stop',
        };

      case 'message_delta':
        if (chunk.delta?.stop_reason) {
          return {
            finishReason: this.convertStopReason(chunk.delta.stop_reason),
          };
        }
        break;
    }

    return null;
  }

  private async makeRequest(request: AnthropicCompletionRequest): Promise<AnthropicCompletionResponse> {
    const response = await fetch(`${this.baseUrl}/v1/messages`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'anthropic-version': this.version,
        'x-api-key': this.apiKey,
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const errorText = await response.text();
      let errorMessage = `Anthropic API error: ${response.status} ${response.statusText}`;

      try {
        const errorData = JSON.parse(errorText);
        if (errorData.error?.message) {
          errorMessage = `Anthropic API error: ${errorData.error.message}`;
        }
      } catch {
        if (errorText) {
          errorMessage = `Anthropic API error: ${errorText}`;
        }
      }

      throw new ModelClientError(
        errorMessage,
        response.status,
        'anthropic',
        this.isRetryableHttpError(response.status)
      );
    }

    const data = await response.json();
    return data;
  }

  private async makeStreamRequest(request: AnthropicCompletionRequest): Promise<Response> {
    const response = await fetch(`${this.baseUrl}/v1/messages`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'anthropic-version': this.version,
        'x-api-key': this.apiKey,
        'Accept': 'text/event-stream',
      },
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const errorText = await response.text();
      let errorMessage = `Anthropic API error: ${response.status} ${response.statusText}`;

      try {
        const errorData = JSON.parse(errorText);
        if (errorData.error?.message) {
          errorMessage = `Anthropic API error: ${errorData.error.message}`;
        }
      } catch {
        if (errorText) {
          errorMessage = `Anthropic API error: ${errorText}`;
        }
      }

      throw new ModelClientError(
        errorMessage,
        response.status,
        'anthropic',
        this.isRetryableHttpError(response.status)
      );
    }

    return response;
  }

  private isRetryableError(error: any): boolean {
    if (error instanceof ModelClientError) {
      return error.retryable;
    }

    // Network errors are generally retryable
    return error.name === 'TypeError' && error.message.includes('fetch');
  }
}