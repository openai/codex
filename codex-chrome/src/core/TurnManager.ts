/**
 * TurnManager implementation - ports run_turn functionality from codex-rs
 * Manages individual conversation turns, handles model streaming, and coordinates tool calls
 */

import { Session, ToolDefinition } from './Session';
import { TurnContext } from './TurnContext';
import { ModelClient, CompletionRequest, CompletionResponse } from '../models/ModelClient';
import { EventMsg, TokenUsage, StreamErrorEvent } from '../protocol/events';
import { Event, InputItem } from '../protocol/types';
import { v4 as uuidv4 } from 'uuid';

/**
 * Result of processing a single response item
 */
export interface ProcessedResponseItem {
  /** The response item from the model */
  item: any;
  /** Optional response that needs to be sent back to model */
  response?: any;
}

/**
 * Result of a complete turn execution
 */
export interface TurnRunResult {
  /** All processed response items from this turn */
  processedItems: ProcessedResponseItem[];
  /** Total token usage for this turn */
  totalTokenUsage?: TokenUsage;
}

/**
 * Configuration for turn execution
 */
export interface TurnConfig {
  /** Maximum number of retry attempts */
  maxRetries?: number;
  /** Base delay between retries in milliseconds */
  retryDelayMs?: number;
  /** Maximum delay between retries in milliseconds */
  maxRetryDelayMs?: number;
}


/**
 * Prompt structure for model requests
 */
export interface Prompt {
  /** Input messages/items for this turn */
  input: any[];
  /** Available tools */
  tools: ToolDefinition[];
  /** Override base instructions */
  baseInstructionsOverride?: string;
}

/**
 * TurnManager handles execution of individual conversation turns
 * Port of run_turn and try_run_turn functions from codex-rs/core/src/codex.rs
 */
export class TurnManager {
  private session: Session;
  private turnContext: TurnContext;
  private modelClient: ModelClient;
  private config: TurnConfig;
  private cancelled = false;

  constructor(
    session: Session,
    turnContext: TurnContext,
    modelClient: ModelClient,
    config: TurnConfig = {}
  ) {
    this.session = session;
    this.turnContext = turnContext;
    this.modelClient = modelClient;
    this.config = {
      maxRetries: 3,
      retryDelayMs: 1000,
      maxRetryDelayMs: 30000,
      ...config,
    };
  }

  /**
   * Cancel the current turn
   */
  cancel(): void {
    this.cancelled = true;
  }

  /**
   * Check if turn is cancelled
   */
  isCancelled(): boolean {
    return this.cancelled;
  }

  /**
   * Run a complete turn with retry logic
   */
  async runTurn(input: any[]): Promise<TurnRunResult> {
    // Build tools list from turn context
    const tools = await this.buildToolsFromContext();

    const prompt: Prompt = {
      input,
      tools,
      baseInstructionsOverride: this.turnContext.getBaseInstructions(),
    };

    let retries = 0;

    while (!this.cancelled) {
      try {
        return await this.tryRunTurn(prompt);
      } catch (error) {
        // Check for non-retryable errors
        if (this.cancelled) {
          throw new Error('Turn cancelled');
        }

        if (this.isNonRetryableError(error)) {
          throw error;
        }

        // Apply retry logic
        if (retries < (this.config.maxRetries || 3)) {
          retries++;
          const delay = this.calculateRetryDelay(retries, error);

          // Notify about retry attempt
          await this.emitStreamError(
            `Stream error: ${error.message}; retrying ${retries}/${this.config.maxRetries} in ${delay}ms`,
            true,
            retries
          );

          await this.sleep(delay);
        } else {
          throw error;
        }
      }
    }

    throw new Error('Turn cancelled');
  }

  /**
   * Attempt to run a turn once (without retry logic)
   */
  private async tryRunTurn(prompt: Prompt): Promise<TurnRunResult> {
    // Record turn context
    await this.recordTurnContext();

    // Process missing call IDs (calls that were interrupted)
    const processedPrompt = this.processMissingCalls(prompt);

    // Create streaming request
    const request = this.buildCompletionRequest(processedPrompt);

    // Start model streaming
    const stream = await this.modelClient.streamCompletion(request);

    const processedItems: ProcessedResponseItem[] = [];
    let totalTokenUsage: TokenUsage | undefined;

    try {
      // Process streaming response
      for await (const event of stream) {
        // Check for cancellation
        if (this.cancelled) {
          throw new Error('Turn cancelled');
        }

        await this.handleStreamEvent(event, processedItems);

        // Capture final token usage
        if (event.type === 'completion' && event.data.usage) {
          totalTokenUsage = this.convertTokenUsage(event.data.usage);
        }
      }

      return {
        processedItems,
        totalTokenUsage,
      };

    } catch (error) {
      // Handle streaming errors
      if (error.message?.includes('stream closed') || error.name === 'StreamError') {
        throw new Error(`Stream error: ${error.message}`);
      }
      throw error;
    }
  }

  /**
   * Build tools list from turn context and session
   */
  private async buildToolsFromContext(): Promise<ToolDefinition[]> {
    const tools: ToolDefinition[] = [];

    // Add core tools based on turn context configuration
    const toolsConfig = this.turnContext.getToolsConfig();

    // Add exec_command tool if enabled
    if (toolsConfig.execCommand !== false) {
      tools.push({
        type: 'function',
        function: {
          name: 'exec_command',
          description: 'Execute a command in the browser context',
          parameters: {
            type: 'object',
            properties: {
              command: { type: 'string', description: 'Command to execute' },
              cwd: { type: 'string', description: 'Working directory' },
            },
            required: ['command'],
          },
        },
      });
    }

    // Add web_search tool if enabled
    if (toolsConfig.webSearch !== false) {
      tools.push({
        type: 'function',
        function: {
          name: 'web_search',
          description: 'Search the web for information',
          parameters: {
            type: 'object',
            properties: {
              query: { type: 'string', description: 'Search query' },
            },
            required: ['query'],
          },
        },
      });
    }

    // Add update_plan tool
    tools.push({
      type: 'function',
      function: {
        name: 'update_plan',
        description: 'Update the current task plan',
        parameters: {
          type: 'object',
          properties: {
            tasks: {
              type: 'array',
              items: {
                type: 'object',
                properties: {
                  id: { type: 'string' },
                  description: { type: 'string' },
                  status: { type: 'string', enum: ['pending', 'in_progress', 'completed'] },
                },
                required: ['id', 'description', 'status'],
              },
            },
          },
          required: ['tasks'],
        },
      },
    });

    // Add MCP tools if available
    const mcpTools = await this.session.getMcpTools();
    tools.push(...mcpTools);

    return tools;
  }

  /**
   * Process missing call IDs and add synthetic aborted responses
   */
  private processMissingCalls(prompt: Prompt): Prompt {
    const completedCallIds = new Set<string>();
    const pendingCallIds = new Set<string>();

    // Collect call IDs
    for (const item of prompt.input) {
      if (item.type === 'function_call_output' && item.call_id) {
        completedCallIds.add(item.call_id);
      }
      if (item.type === 'function_call' && item.call_id) {
        pendingCallIds.add(item.call_id);
      }
    }

    // Find missing calls
    const missingCallIds = [...pendingCallIds].filter(id => !completedCallIds.has(id));

    if (missingCallIds.length === 0) {
      return prompt;
    }

    // Add synthetic aborted responses for missing calls
    const syntheticResponses = missingCallIds.map(callId => ({
      type: 'function_call_output',
      call_id: callId,
      content: 'aborted',
      success: false,
    }));

    return {
      ...prompt,
      input: [...syntheticResponses, ...prompt.input],
    };
  }

  /**
   * Build completion request for model client
   */
  private buildCompletionRequest(prompt: Prompt): CompletionRequest {
    return {
      model: this.turnContext.getModel(),
      messages: this.convertPromptToMessages(prompt),
      tools: prompt.tools,
      stream: true,
      temperature: 0.7,
      maxTokens: 4096,
    };
  }

  /**
   * Convert prompt format to model client message format
   */
  private convertPromptToMessages(prompt: Prompt): any[] {
    const messages: any[] = [];

    // Add base instructions if provided
    if (prompt.baseInstructionsOverride) {
      messages.push({
        role: 'system',
        content: prompt.baseInstructionsOverride,
      });
    }

    // Convert input items to messages
    for (const item of prompt.input) {
      if (item.role && item.content) {
        messages.push({
          role: item.role,
          content: item.content,
          toolCalls: item.toolCalls,
          toolCallId: item.toolCallId,
        });
      }
    }

    return messages;
  }

  /**
   * Handle individual stream events
   */
  private async handleStreamEvent(
    event: any,
    processedItems: ProcessedResponseItem[]
  ): Promise<void> {
    switch (event.type) {
      case 'content_delta':
        await this.handleContentDelta(event.data);
        break;

      case 'tool_call':
        await this.handleToolCall(event.data, processedItems);
        break;

      case 'message_complete':
        await this.handleMessageComplete(event.data, processedItems);
        break;

      case 'error':
        throw new Error(`Stream error: ${event.data.message}`);

      default:
        // Unknown event type, log but don't fail
        console.warn('Unknown stream event type:', event.type);
    }
  }

  /**
   * Handle content delta events (streaming text)
   */
  private async handleContentDelta(data: any): Promise<void> {
    await this.emitEvent({
      type: 'AgentMessageDelta',
      data: {
        delta: data.text || data.content || '',
      },
    });
  }

  /**
   * Handle tool call events
   */
  private async handleToolCall(data: any, processedItems: ProcessedResponseItem[]): Promise<void> {
    const { toolName, parameters, callId } = data;

    // Create the tool call item
    const toolCallItem = {
      type: 'function_call',
      call_id: callId,
      name: toolName,
      parameters,
    };

    // Execute the tool call
    const toolResponse = await this.executeToolCall(toolName, parameters, callId);

    processedItems.push({
      item: toolCallItem,
      response: toolResponse,
    });
  }

  /**
   * Handle message completion events
   */
  private async handleMessageComplete(data: any, processedItems: ProcessedResponseItem[]): Promise<void> {
    if (data.role === 'assistant' && data.content) {
      // This is a final assistant message (no tool calls)
      const messageItem = {
        role: 'assistant',
        content: data.content,
      };

      processedItems.push({
        item: messageItem,
        response: undefined, // No response needed for final messages
      });

      // Emit the complete message
      await this.emitEvent({
        type: 'AgentMessage',
        data: {
          message: data.content,
        },
      });
    }
  }

  /**
   * Execute a tool call and return the response
   */
  private async executeToolCall(toolName: string, parameters: any, callId: string): Promise<any> {
    try {
      let result: any;

      switch (toolName) {
        case 'exec_command':
          result = await this.executeCommand(parameters.command, parameters.cwd);
          break;

        case 'web_search':
          result = await this.executeWebSearch(parameters.query);
          break;

        case 'update_plan':
          result = await this.updatePlan(parameters.tasks);
          break;

        default:
          // Try MCP tools
          result = await this.executeMcpTool(toolName, parameters);
          break;
      }

      return {
        type: 'function_call_output',
        call_id: callId,
        content: JSON.stringify(result),
        success: true,
      };

    } catch (error) {
      return {
        type: 'function_call_output',
        call_id: callId,
        content: `Error: ${error.message}`,
        success: false,
      };
    }
  }

  /**
   * Execute command in browser context
   */
  private async executeCommand(command: string, cwd?: string): Promise<any> {
    // Emit command begin event
    await this.emitEvent({
      type: 'ExecCommandBegin',
      data: {
        session_id: this.session.getSessionId(),
        command,
        tab_id: await this.getCurrentTabId(),
        url: await this.getCurrentUrl(),
      },
    });

    try {
      // In browser context, this would interact with page content
      // For now, return a placeholder response
      const result = {
        stdout: `Executed: ${command}`,
        stderr: '',
        exit_code: 0,
      };

      // Emit command end event
      await this.emitEvent({
        type: 'ExecCommandEnd',
        data: {
          session_id: this.session.getSessionId(),
          exit_code: result.exit_code,
        },
      });

      return result;

    } catch (error) {
      await this.emitEvent({
        type: 'ExecCommandEnd',
        data: {
          session_id: this.session.getSessionId(),
          exit_code: 1,
        },
      });
      throw error;
    }
  }

  /**
   * Execute web search
   */
  private async executeWebSearch(query: string): Promise<any> {
    await this.emitEvent({
      type: 'WebSearchBegin',
      data: { query },
    });

    try {
      // Placeholder web search implementation
      const results = {
        query,
        results: [
          { title: 'Sample Result', url: 'https://example.com', snippet: 'Sample snippet' },
        ],
      };

      await this.emitEvent({
        type: 'WebSearchEnd',
        data: {
          query,
          results_count: results.results.length,
        },
      });

      return results;
    } catch (error) {
      await this.emitEvent({
        type: 'WebSearchEnd',
        data: {
          query,
          results_count: 0,
        },
      });
      throw error;
    }
  }

  /**
   * Update task plan
   */
  private async updatePlan(tasks: any[]): Promise<any> {
    await this.emitEvent({
      type: 'PlanUpdate',
      data: { tasks },
    });

    return { success: true, tasks };
  }

  /**
   * Execute MCP tool
   */
  private async executeMcpTool(toolName: string, parameters: any): Promise<any> {
    await this.emitEvent({
      type: 'McpToolCallBegin',
      data: {
        tool_name: toolName,
        params: parameters,
      },
    });

    try {
      const result = await this.session.executeMcpTool(toolName, parameters);

      await this.emitEvent({
        type: 'McpToolCallEnd',
        data: {
          tool_name: toolName,
          result,
        },
      });

      return result;
    } catch (error) {
      await this.emitEvent({
        type: 'McpToolCallEnd',
        data: {
          tool_name: toolName,
          error: error.message,
        },
      });
      throw error;
    }
  }

  /**
   * Record turn context for rollout/history
   */
  private async recordTurnContext(): Promise<void> {
    const turnContextItem = {
      cwd: this.turnContext.getCwd(),
      approval_policy: this.turnContext.getApprovalPolicy(),
      sandbox_policy: this.turnContext.getSandboxPolicy(),
      model: this.turnContext.getModel(),
      effort: this.turnContext.getEffort(),
      summary: this.turnContext.getSummary(),
    };

    await this.session.recordTurnContext(turnContextItem);
  }

  /**
   * Check if error is non-retryable
   */
  private isNonRetryableError(error: any): boolean {
    const message = error.message?.toLowerCase() || '';
    return (
      message.includes('interrupted') ||
      message.includes('cancelled') ||
      message.includes('usage limit') ||
      message.includes('unauthorized') ||
      error.name === 'AuthenticationError'
    );
  }

  /**
   * Calculate retry delay with exponential backoff
   */
  private calculateRetryDelay(attempt: number, error: any): number {
    // Check if error specifies a delay
    if (error.retryAfter) {
      return Math.min(error.retryAfter * 1000, this.config.maxRetryDelayMs || 30000);
    }

    // Exponential backoff
    const baseDelay = this.config.retryDelayMs || 1000;
    const exponentialDelay = baseDelay * Math.pow(2, attempt - 1);
    const maxDelay = this.config.maxRetryDelayMs || 30000;

    return Math.min(exponentialDelay, maxDelay);
  }

  /**
   * Convert model client token usage to protocol format
   */
  private convertTokenUsage(usage: any): TokenUsage {
    return {
      input_tokens: usage.prompt_tokens || 0,
      cached_input_tokens: usage.cached_tokens || 0,
      output_tokens: usage.completion_tokens || 0,
      reasoning_output_tokens: usage.reasoning_tokens || 0,
      total_tokens: usage.total_tokens || 0,
    };
  }

  /**
   * Get current browser tab ID
   */
  private async getCurrentTabId(): Promise<number | undefined> {
    if (typeof chrome !== 'undefined' && chrome.tabs) {
      try {
        const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
        return tab?.id;
      } catch (error) {
        console.warn('Failed to get current tab ID:', error);
      }
    }
    return undefined;
  }

  /**
   * Get current page URL
   */
  private async getCurrentUrl(): Promise<string | undefined> {
    if (typeof chrome !== 'undefined' && chrome.tabs) {
      try {
        const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
        return tab?.url;
      } catch (error) {
        console.warn('Failed to get current URL:', error);
      }
    }
    return undefined;
  }

  /**
   * Emit an event through the session
   */
  private async emitEvent(msg: EventMsg): Promise<void> {
    const event: Event = {
      id: uuidv4(),
      msg,
    };
    await this.session.emitEvent(event);
  }

  /**
   * Emit stream error event
   */
  private async emitStreamError(error: string, retrying: boolean, attempt?: number): Promise<void> {
    await this.emitEvent({
      type: 'StreamError',
      data: {
        error,
        retrying,
        attempt,
      },
    });
  }

  /**
   * Sleep utility for retry delays
   */
  private sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}