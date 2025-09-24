/**
 * OpenAI API client implementation for codex-chrome
 * Implements the ModelClient interface with OpenAI-specific functionality
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
 * OpenAI-specific request format
 */
interface OpenAICompletionRequest {
  model: string;
  messages: OpenAIMessage[];
  temperature?: number;
  max_tokens?: number;
  tools?: OpenAITool[];
  stream?: boolean;
}

/**
 * OpenAI message format
 */
interface OpenAIMessage {
  role: 'system' | 'user' | 'assistant' | 'tool';
  content?: string | null;
  tool_calls?: OpenAIToolCall[];
  tool_call_id?: string;
}

/**
 * OpenAI tool definition format
 */
interface OpenAITool {
  type: 'function';
  function: {
    name: string;
    description: string;
    parameters: any;
  };
}

/**
 * OpenAI tool call format
 */
interface OpenAIToolCall {
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string;
  };
}

/**
 * OpenAI completion response format
 */
interface OpenAICompletionResponse {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: OpenAIChoice[];
  usage: OpenAIUsage;
}

/**
 * OpenAI choice format
 */
interface OpenAIChoice {
  index: number;
  message: OpenAIMessage;
  finish_reason: 'stop' | 'length' | 'tool_calls' | 'content_filter';
}

/**
 * OpenAI usage format
 */
interface OpenAIUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

/**
 * OpenAI streaming chunk format
 */
interface OpenAIStreamChunk {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: Array<{
    index: number;
    delta: {
      role?: string;
      content?: string;
      tool_calls?: OpenAIToolCall[];
    };
    finish_reason?: 'stop' | 'length' | 'tool_calls' | 'content_filter';
  }>;
}

/**
 * Token counting mappings for different models
 */
const TOKEN_MULTIPLIERS: Record<string, number> = {
  'gpt-4': 1.3,
  'gpt-4-turbo': 1.3,
  'gpt-4o': 1.2,
  'gpt-3.5-turbo': 1.5,
  'gpt-3.5-turbo-16k': 1.5,
};

/**
 * OpenAI API client implementation
 */
export class OpenAIClient extends ModelClient {
  private readonly apiKey: string;
  private readonly baseUrl: string;
  private readonly organization?: string;

  constructor(
    apiKey: string,
    options: {
      baseUrl?: string;
      organization?: string;
    } = {}
  ) {
    super();

    if (!apiKey?.trim()) {
      throw new ModelClientError('OpenAI API key is required');
    }

    this.apiKey = apiKey;
    this.baseUrl = options.baseUrl || 'https://api.openai.com/v1';
    this.organization = options.organization;
  }

  getProvider(): string {
    return 'openai';
  }

  async complete(request: CompletionRequest): Promise<CompletionResponse> {
    this.validateRequest(request);

    const openaiRequest = this.convertToOpenAIRequest(request);

    const response = await this.withRetry(
      () => this.makeRequest(openaiRequest),
      (error) => this.isRetryableError(error)
    );

    return this.convertFromOpenAIResponse(response);
  }

  async *stream(request: CompletionRequest): AsyncGenerator<StreamChunk> {
    this.validateRequest(request);

    const openaiRequest = this.convertToOpenAIRequest({ ...request, stream: true });

    const response = await this.withRetry(
      () => this.makeStreamRequest(openaiRequest),
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

          if (data === '[DONE]') {
            return;
          }

          try {
            const chunk: OpenAIStreamChunk = JSON.parse(data);
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
    // Simple token counting approximation
    // In a real implementation, this would use tiktoken or similar
    const multiplier = TOKEN_MULTIPLIERS[model] || 1.4;
    const words = text.split(/\s+/).length;
    const punctuation = (text.match(/[.!?;:,]/g) || []).length;

    return Math.ceil((words + punctuation * 0.5) * multiplier);
  }

  private convertToOpenAIRequest(request: CompletionRequest): OpenAICompletionRequest {
    return {
      model: request.model,
      messages: request.messages.map(this.convertMessage),
      temperature: request.temperature,
      max_tokens: request.maxTokens,
      tools: request.tools?.map(tool => ({
        type: tool.type,
        function: tool.function,
      })),
      stream: request.stream,
    };
  }

  private convertMessage(message: Message): OpenAIMessage {
    return {
      role: message.role,
      content: message.content,
      tool_calls: message.toolCalls?.map(toolCall => ({
        id: toolCall.id,
        type: toolCall.type,
        function: toolCall.function,
      })),
      tool_call_id: message.toolCallId,
    };
  }

  private convertFromOpenAIResponse(response: OpenAICompletionResponse): CompletionResponse {
    return {
      id: response.id,
      model: response.model,
      choices: response.choices.map(choice => ({
        index: choice.index,
        message: {
          role: choice.message.role,
          content: choice.message.content,
          toolCalls: choice.message.tool_calls?.map(toolCall => ({
            id: toolCall.id,
            type: toolCall.type,
            function: toolCall.function,
          })),
          toolCallId: choice.message.tool_call_id,
        },
        finishReason: choice.finish_reason,
      })),
      usage: {
        promptTokens: response.usage.prompt_tokens,
        completionTokens: response.usage.completion_tokens,
        totalTokens: response.usage.total_tokens,
      },
    };
  }

  private convertStreamChunk(chunk: OpenAIStreamChunk): StreamChunk | null {
    const choice = chunk.choices[0];

    if (!choice) {
      return null;
    }

    const streamChunk: StreamChunk = {};

    if (choice.delta) {
      streamChunk.delta = {};

      if (choice.delta.content) {
        streamChunk.delta.content = choice.delta.content;
      }

      if (choice.delta.tool_calls) {
        streamChunk.delta.toolCalls = choice.delta.tool_calls.map(toolCall => ({
          id: toolCall.id,
          type: toolCall.type,
          function: toolCall.function,
        }));
      }
    }

    if (choice.finish_reason) {
      streamChunk.finishReason = choice.finish_reason;
    }

    return streamChunk;
  }

  private async makeRequest(request: OpenAICompletionRequest): Promise<OpenAICompletionResponse> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${this.apiKey}`,
    };

    if (this.organization) {
      headers['OpenAI-Organization'] = this.organization;
    }

    const response = await fetch(`${this.baseUrl}/chat/completions`, {
      method: 'POST',
      headers,
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const errorText = await response.text();
      let errorMessage = `OpenAI API error: ${response.status} ${response.statusText}`;

      try {
        const errorData = JSON.parse(errorText);
        if (errorData.error?.message) {
          errorMessage = `OpenAI API error: ${errorData.error.message}`;
        }
      } catch {
        // Use the raw error text if JSON parsing fails
        if (errorText) {
          errorMessage = `OpenAI API error: ${errorText}`;
        }
      }

      throw new ModelClientError(
        errorMessage,
        response.status,
        'openai',
        this.isRetryableHttpError(response.status)
      );
    }

    const data = await response.json();
    return data;
  }

  private async makeStreamRequest(request: OpenAICompletionRequest): Promise<Response> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${this.apiKey}`,
      'Accept': 'text/event-stream',
    };

    if (this.organization) {
      headers['OpenAI-Organization'] = this.organization;
    }

    const response = await fetch(`${this.baseUrl}/chat/completions`, {
      method: 'POST',
      headers,
      body: JSON.stringify(request),
    });

    if (!response.ok) {
      const errorText = await response.text();
      let errorMessage = `OpenAI API error: ${response.status} ${response.statusText}`;

      try {
        const errorData = JSON.parse(errorText);
        if (errorData.error?.message) {
          errorMessage = `OpenAI API error: ${errorData.error.message}`;
        }
      } catch {
        if (errorText) {
          errorMessage = `OpenAI API error: ${errorText}`;
        }
      }

      throw new ModelClientError(
        errorMessage,
        response.status,
        'openai',
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