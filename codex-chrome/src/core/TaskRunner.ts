/**
 * TaskRunner implementation - ports run_task functionality from codex-rs
 * Manages task execution lifecycle, handles task cancellation, and emits progress events
 */

import { Session } from './Session';
import { TurnManager } from './TurnManager';
import { TurnContext } from './TurnContext';
import { InputItem, Event } from '../protocol/types';
import { EventMsg, TaskStartedEvent, TaskCompleteEvent, ErrorEvent, TurnAbortedEvent } from '../protocol/events';
import { v4 as uuidv4 } from 'uuid';

/**
 * Task execution result
 */
export interface TaskResult {
  success: boolean;
  lastAgentMessage?: string;
  error?: string;
  aborted?: boolean;
}

/**
 * Task execution options
 */
export interface TaskOptions {
  /** Enable review mode for isolated execution */
  reviewMode?: boolean;
  /** Task timeout in milliseconds */
  timeoutMs?: number;
  /** Auto-compact when token limit reached */
  autoCompact?: boolean;
}

/**
 * TaskRunner handles the execution of a complete task which may involve multiple turns
 * Port of run_task function from codex-rs/core/src/codex.rs
 */
export class TaskRunner {
  private session: Session;
  private turnContext: TurnContext;
  private turnManager: TurnManager;
  private submissionId: string;
  private input: InputItem[];
  private options: TaskOptions;
  private cancelled = false;
  private cancelPromise: Promise<void> | null = null;
  private cancelResolve: (() => void) | null = null;

  constructor(
    session: Session,
    turnContext: TurnContext,
    turnManager: TurnManager,
    submissionId: string,
    input: InputItem[],
    options: TaskOptions = {}
  ) {
    this.session = session;
    this.turnContext = turnContext;
    this.turnManager = turnManager;
    this.submissionId = submissionId;
    this.input = input;
    this.options = options;

    // Set up cancellation mechanism
    this.cancelPromise = new Promise<void>((resolve) => {
      this.cancelResolve = resolve;
    });
  }

  /**
   * Cancel the running task
   */
  cancel(): void {
    this.cancelled = true;
    this.turnManager.cancel();
    if (this.cancelResolve) {
      this.cancelResolve();
    }
  }

  /**
   * Check if task is cancelled
   */
  isCancelled(): boolean {
    return this.cancelled;
  }

  /**
   * Run the task - main execution method
   */
  async run(): Promise<TaskResult> {
    try {
      // Empty input check
      if (this.input.length === 0) {
        return { success: true };
      }

      // Emit TaskStarted event
      await this.emitEvent({
        type: 'TaskStarted',
        data: {
          model_context_window: this.turnContext.getModelContextWindow(),
        },
      });

      // Handle review mode vs normal mode
      let reviewThreadHistory: any[] = [];
      if (this.options.reviewMode) {
        // Initialize isolated review thread history
        reviewThreadHistory = await this.buildInitialReviewContext();
        reviewThreadHistory.push(this.convertInputToResponseItem());
      } else {
        // Record input in session history for normal mode
        await this.session.recordInput(this.input);
      }

      let lastAgentMessage: string | undefined;
      let autoCompactAttempted = false;

      // Main task execution loop
      while (!this.cancelled) {
        // Check for pending user input during execution
        const pendingInput = await this.session.getPendingInput();

        // Prepare turn input based on mode
        const turnInput = this.options.reviewMode
          ? this.buildReviewTurnInput(reviewThreadHistory, pendingInput)
          : await this.buildNormalTurnInput(pendingInput);

        // Check cancellation before turn
        if (this.cancelled) {
          await this.emitAbortedEvent('user_interrupt');
          return { success: false, aborted: true };
        }

        try {
          // Execute turn
          const turnResult = await this.runTurnWithTimeout(turnInput);

          // Process turn results
          const processResult = await this.processTurnResult(
            turnResult,
            reviewThreadHistory
          );

          lastAgentMessage = processResult.lastAgentMessage || lastAgentMessage;

          // Check if task is complete (no pending tool calls)
          if (processResult.taskComplete) {
            break;
          }

          // Handle token limit and auto-compact
          if (processResult.tokenLimitReached && !autoCompactAttempted) {
            autoCompactAttempted = true;
            await this.attemptAutoCompact();
          }

        } catch (error) {
          // Handle turn errors with retry logic or escalation
          const shouldRetry = await this.handleTurnError(error);
          if (!shouldRetry) {
            throw error;
          }
        }
      }

      // Task completed successfully
      await this.emitEvent({
        type: 'TaskComplete',
        data: {
          last_agent_message: lastAgentMessage,
        },
      });

      return {
        success: true,
        lastAgentMessage,
      };

    } catch (error) {
      // Handle task-level errors
      const errorMessage = error instanceof Error ? error.message : String(error);

      await this.emitEvent({
        type: 'Error',
        data: {
          message: `Task execution failed: ${errorMessage}`,
        },
      });

      return {
        success: false,
        error: errorMessage,
      };
    }
  }

  /**
   * Run a turn with timeout support
   */
  private async runTurnWithTimeout(turnInput: any[]): Promise<any> {
    const timeout = this.options.timeoutMs;
    if (!timeout) {
      return this.turnManager.runTurn(turnInput);
    }

    return Promise.race([
      this.turnManager.runTurn(turnInput),
      new Promise((_, reject) => {
        setTimeout(() => reject(new Error('Turn timeout')), timeout);
      }),
      this.cancelPromise?.then(() => {
        throw new Error('Task cancelled');
      }),
    ]);
  }

  /**
   * Build initial review context for review mode
   */
  private async buildInitialReviewContext(): Promise<any[]> {
    // Build environment context similar to Rust implementation
    return [
      {
        role: 'system',
        content: [
          {
            type: 'text',
            text: `Working directory: ${this.turnContext.getCwd()}`,
          },
        ],
      },
    ];
  }

  /**
   * Convert input items to response format
   */
  private convertInputToResponseItem(): any {
    return {
      role: 'user',
      content: this.input.map(item => {
        switch (item.type) {
          case 'text':
            return { type: 'text', text: item.text };
          case 'image':
            return { type: 'image', image_url: item.image_url };
          case 'clipboard':
            return { type: 'text', text: item.content || '[clipboard content]' };
          case 'context':
            return { type: 'text', text: `[context: ${item.path || 'unknown'}]` };
          default:
            return { type: 'text', text: '[unknown input]' };
        }
      }),
    };
  }

  /**
   * Build turn input for review mode
   */
  private buildReviewTurnInput(reviewHistory: any[], pendingInput: any[]): any[] {
    const turnInput = [...reviewHistory];
    if (pendingInput.length > 0) {
      turnInput.push(...pendingInput);
    }
    return turnInput;
  }

  /**
   * Build turn input for normal mode
   */
  private async buildNormalTurnInput(pendingInput: any[]): Promise<any[]> {
    if (pendingInput.length > 0) {
      await this.session.recordConversationItems(pendingInput);
    }
    return this.session.buildTurnInputWithHistory(pendingInput);
  }

  /**
   * Process the results of a turn execution
   */
  private async processTurnResult(
    turnResult: any,
    reviewHistory: any[]
  ): Promise<{
    taskComplete: boolean;
    tokenLimitReached: boolean;
    lastAgentMessage?: string;
  }> {
    const { processedItems, totalTokenUsage } = turnResult;

    let taskComplete = true;
    let lastAgentMessage: string | undefined;
    const itemsToRecord: any[] = [];

    // Process each response item
    for (const processedItem of processedItems) {
      const { item, response } = processedItem;

      // Check if this is an assistant message (task completion indicator)
      if (item.role === 'assistant' && !response) {
        lastAgentMessage = this.extractTextContent(item);
        itemsToRecord.push(item);
      }
      // Check if this is a tool call that needs response (task continues)
      else if (response) {
        taskComplete = false;
        itemsToRecord.push(item);
        if (response.role === 'tool') {
          itemsToRecord.push(response);
        }
      }
    }

    // Record processed items in conversation history
    if (this.options.reviewMode) {
      // Add to isolated review history
      reviewHistory.push(...itemsToRecord);
    } else {
      // Add to session history
      await this.session.recordConversationItems(itemsToRecord);
    }

    // Check token limits
    const contextWindow = this.turnContext.getModelContextWindow();
    const tokenLimitReached = totalTokenUsage && contextWindow
      ? totalTokenUsage.total_tokens >= contextWindow * 0.9 // 90% threshold
      : false;

    return {
      taskComplete,
      tokenLimitReached,
      lastAgentMessage,
    };
  }

  /**
   * Extract text content from a message item
   */
  private extractTextContent(item: any): string | undefined {
    if (!item.content || !Array.isArray(item.content)) {
      return undefined;
    }

    return item.content
      .filter((content: any) => content.type === 'text')
      .map((content: any) => content.text)
      .join(' ');
  }

  /**
   * Handle errors during turn execution
   */
  private async handleTurnError(error: any): Promise<boolean> {
    // Check if it's a cancellation
    if (this.cancelled || error.message?.includes('cancelled')) {
      await this.emitAbortedEvent('user_interrupt');
      return false;
    }

    // Check if it's a retryable error
    if (this.isRetryableError(error)) {
      // TurnManager will handle retries internally
      return true;
    }

    // Non-retryable error
    return false;
  }

  /**
   * Check if an error is retryable
   */
  private isRetryableError(error: any): boolean {
    const message = error.message?.toLowerCase() || '';
    return (
      message.includes('stream') ||
      message.includes('network') ||
      message.includes('timeout') ||
      error.name === 'NetworkError'
    );
  }

  /**
   * Attempt automatic compaction when token limit is reached
   */
  private async attemptAutoCompact(): Promise<void> {
    try {
      await this.session.compact();
    } catch (error) {
      // Log but don't fail the task
      console.warn('Auto-compact failed:', error);
    }
  }

  /**
   * Emit an event through the session's event queue
   */
  private async emitEvent(msg: EventMsg): Promise<void> {
    const event: Event = {
      id: this.submissionId,
      msg,
    };
    await this.session.emitEvent(event);
  }

  /**
   * Emit task aborted event
   */
  private async emitAbortedEvent(reason: 'user_interrupt' | 'automatic_abort' | 'error'): Promise<void> {
    await this.emitEvent({
      type: 'TurnAborted',
      data: {
        reason,
        submission_id: this.submissionId,
      },
    });
  }

  /**
   * Static factory method to create and run a task
   */
  static async runTask(
    session: Session,
    turnContext: TurnContext,
    turnManager: TurnManager,
    submissionId: string,
    input: InputItem[],
    options?: TaskOptions
  ): Promise<TaskResult> {
    const taskRunner = new TaskRunner(
      session,
      turnContext,
      turnManager,
      submissionId,
      input,
      options
    );

    return taskRunner.run();
  }
}