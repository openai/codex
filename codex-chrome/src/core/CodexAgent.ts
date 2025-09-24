/**
 * Main Codex agent class - port of codex.rs Codex struct
 * Preserves the SQ/EQ (Submission Queue/Event Queue) architecture
 */

import type { Submission, Op, Event } from '../protocol/types';
import type { EventMsg } from '../protocol/events';
import { Session } from './Session';
import { TaskRunner } from './TaskRunner';
import { TurnManager } from './TurnManager';
import { ApprovalManager } from './ApprovalManager';
import { DiffTracker } from './DiffTracker';
import { ToolRegistry } from '../tools/ToolRegistry';
import { ModelClientFactory } from '../models/ModelClientFactory';
import { v4 as uuidv4 } from 'uuid';

/**
 * Main agent class managing the submission and event queues
 */
export class CodexAgent {
  private nextId: number = 1;
  private submissionQueue: Submission[] = [];
  private eventQueue: Event[] = [];
  private session: Session;
  private isProcessing: boolean = false;
  // These will be initialized per task/turn as needed
  // private taskRunner: TaskRunner;
  // private turnManager: TurnManager;
  private approvalManager: ApprovalManager;
  private diffTracker: DiffTracker;
  private toolRegistry: ToolRegistry;
  private modelClientFactory: ModelClientFactory;

  constructor() {
    this.session = new Session();
    this.modelClientFactory = ModelClientFactory.getInstance();
    this.toolRegistry = new ToolRegistry();
    this.approvalManager = new ApprovalManager();
    this.diffTracker = new DiffTracker();
    // Components are initialized but not fully integrated yet
    // Full integration pending interface alignment
  }

  /**
   * Submit an operation to the agent
   * Returns the submission ID
   */
  async submitOperation(op: Op): Promise<string> {
    const id = `sub_${this.nextId++}`;
    const submission: Submission = { id, op };

    this.submissionQueue.push(submission);

    // Start processing if not already running
    if (!this.isProcessing) {
      this.processSubmissionQueue();
    }

    return id;
  }

  /**
   * Get the next event from the event queue
   */
  async getNextEvent(): Promise<Event | null> {
    return this.eventQueue.shift() || null;
  }

  /**
   * Process submissions from the queue
   */
  private async processSubmissionQueue(): Promise<void> {
    this.isProcessing = true;

    while (this.submissionQueue.length > 0) {
      const submission = this.submissionQueue.shift()!;

      try {
        await this.handleSubmission(submission);
      } catch (error) {
        this.emitEvent({
          type: 'Error',
          data: {
            message: error instanceof Error ? error.message : 'Unknown error occurred',
          },
        });
      }
    }

    this.isProcessing = false;
  }

  /**
   * Handle a single submission
   */
  private async handleSubmission(submission: Submission): Promise<void> {
    // Emit TaskStarted event
    this.emitEvent({
      type: 'TaskStarted',
      data: {
        model_context_window: undefined, // Will be set when model is connected
      },
    });

    try {
      switch (submission.op.type) {
        case 'Interrupt':
          await this.handleInterrupt();
          break;

        case 'UserInput':
          await this.handleUserInput(submission.op);
          break;

        case 'UserTurn':
          await this.handleUserTurn(submission.op);
          break;

        case 'OverrideTurnContext':
          await this.handleOverrideTurnContext(submission.op);
          break;

        case 'ExecApproval':
          await this.handleExecApproval(submission.op);
          break;

        case 'PatchApproval':
          await this.handlePatchApproval(submission.op);
          break;

        case 'AddToHistory':
          await this.handleAddToHistory(submission.op);
          break;

        case 'GetPath':
          await this.handleGetPath();
          break;

        case 'Shutdown':
          await this.handleShutdown();
          break;

        default:
          // Handle other op types
          this.emitEvent({
            type: 'AgentMessage',
            data: {
              message: `Operation type ${(submission.op as any).type} not yet implemented`,
            },
          });
      }

      // Emit TaskComplete event
      this.emitEvent({
        type: 'TaskComplete',
        data: {
          last_agent_message: undefined,
        },
      });
    } catch (error) {
      // Emit TurnAborted event on error
      this.emitEvent({
        type: 'TurnAborted',
        data: {
          reason: 'error',
          submission_id: submission.id,
        },
      });
      throw error;
    }
  }

  /**
   * Handle interrupt operation
   */
  private async handleInterrupt(): Promise<void> {
    // Clear the submission queue
    this.submissionQueue = [];

    this.emitEvent({
      type: 'TurnAborted',
      data: {
        reason: 'user_interrupt',
      },
    });
  }

  /**
   * Handle user input
   */
  private async handleUserInput(op: Extract<Op, { type: 'UserInput' }>): Promise<void> {
    // Process user input items
    for (const item of op.items) {
      if (item.type === 'text') {
        // For now, just echo back the text
        this.emitEvent({
          type: 'AgentMessage',
          data: {
            message: `Processing: ${item.text}`,
          },
        });
      }
    }
  }

  /**
   * Handle user turn with full context
   */
  private async handleUserTurn(op: Extract<Op, { type: 'UserTurn' }>): Promise<void> {
    try {
      // Update session turn context
      this.session.updateTurnContext({
        cwd: op.cwd,
        approval_policy: op.approval_policy,
        sandbox_policy: op.sandbox_policy,
        model: op.model,
        effort: op.effort,
        summary: op.summary,
      });

      // Process the input items through the session
      for (const item of op.items) {
        if (item.type === 'text') {
          this.emitEvent({
            type: 'AgentMessage',
            data: {
              message: `Processing: ${item.text}`,
            },
          });
        }
      }

      // For now, emit a simple completion
      // TODO: Integrate with TaskRunner, TurnManager, ApprovalManager, and DiffTracker
      // once the interfaces are properly aligned
      this.emitEvent({
        type: 'AgentMessage',
        data: {
          message: 'Task completed successfully. Full integration with new components is in progress.',
        },
      });

    } catch (error) {
      console.error('Error in handleUserTurn:', error);

      this.emitEvent({
        type: 'Error',
        data: {
          message: error instanceof Error ? error.message : 'Unknown error occurred during task execution',
        },
      });

      throw error;
    }
  }

  /**
   * Handle override turn context
   */
  private async handleOverrideTurnContext(
    op: Extract<Op, { type: 'OverrideTurnContext' }>
  ): Promise<void> {
    // Partial update of turn context
    const updates: any = {};

    if (op.cwd !== undefined) updates.cwd = op.cwd;
    if (op.approval_policy !== undefined) updates.approval_policy = op.approval_policy;
    if (op.sandbox_policy !== undefined) updates.sandbox_policy = op.sandbox_policy;
    if (op.model !== undefined) updates.model = op.model;
    if (op.effort !== undefined) updates.effort = op.effort;
    if (op.summary !== undefined) updates.summary = op.summary;

    this.session.updateTurnContext(updates);
  }

  /**
   * Handle exec approval
   */
  private async handleExecApproval(op: Extract<Op, { type: 'ExecApproval' }>): Promise<void> {
    // For now, just log the approval - proper implementation would integrate with the approval system
    console.log(`Approval ${op.decision === 'approve' ? 'granted' : 'denied'} for ${op.id}`);

    // Emit event
    this.emitEvent({
      type: 'BackgroundEvent',
      data: {
        message: `Execution ${op.decision === 'approve' ? 'approved' : 'rejected'}: ${op.id}`,
        level: 'info',
      },
    });
  }

  /**
   * Handle patch approval
   */
  private async handlePatchApproval(op: Extract<Op, { type: 'PatchApproval' }>): Promise<void> {
    // For now, just log the approval - proper implementation would integrate with the diff system
    console.log(`Patch ${op.decision === 'approve' ? 'approved' : 'rejected'} for ${op.id}`);

    // Emit event
    this.emitEvent({
      type: 'BackgroundEvent',
      data: {
        message: `Patch ${op.decision === 'approve' ? 'approved' : 'rejected'}: ${op.id}`,
        level: 'info',
      },
    });
  }

  /**
   * Handle add to history
   */
  private async handleAddToHistory(op: Extract<Op, { type: 'AddToHistory' }>): Promise<void> {
    this.session.addToHistory({
      timestamp: Date.now(),
      text: op.text,
      type: 'user',
    });
  }

  /**
   * Handle get path request
   */
  private async handleGetPath(): Promise<void> {
    const history = this.session.getHistory();
    this.emitEvent({
      type: 'ConversationPath',
      data: {
        path: this.session.conversationId,
        messages_count: history.length,
      },
    });
  }

  /**
   * Handle shutdown
   */
  private async handleShutdown(): Promise<void> {
    // Clean up and emit shutdown complete
    this.submissionQueue = [];
    this.eventQueue = [];

    this.emitEvent({
      type: 'ShutdownComplete',
    });
  }

  /**
   * Emit an event to the event queue
   */
  private emitEvent(msg: EventMsg): void {
    const event: Event = {
      id: `evt_${this.nextId++}`,
      msg,
    };

    this.eventQueue.push(event);

    // Notify listeners via Chrome runtime if available
    if (typeof chrome !== 'undefined' && chrome.runtime) {
      chrome.runtime.sendMessage({
        type: 'EVENT',
        payload: event,
      }).catch(() => {
        // Ignore errors if no listeners
      });
    }
  }

  /**
   * Get the current session
   */
  getSession(): Session {
    return this.session;
  }

  /**
   * Get the task runner (creates new instance per task)
   */
  getTaskRunner(): void {
    // TaskRunner is created per task, not stored as instance property
    throw new Error('TaskRunner instances are created per task execution');
  }

  /**
   * Get the tool registry
   */
  getToolRegistry(): ToolRegistry {
    return this.toolRegistry;
  }

  /**
   * Get the approval manager
   */
  getApprovalManager(): ApprovalManager {
    return this.approvalManager;
  }

  /**
   * Get the diff tracker
   */
  getDiffTracker(): DiffTracker {
    return this.diffTracker;
  }

  /**
   * Cleanup resources
   */
  async cleanup(): Promise<void> {
    this.toolRegistry.clear();
    this.submissionQueue = [];
    this.eventQueue = [];
  }
}