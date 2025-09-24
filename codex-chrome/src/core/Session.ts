/**
 * Session management class - port of Session struct from codex.rs
 * Manages conversation state, turn context, and history
 */

import { InputItem, AskForApproval, SandboxPolicy, ReasoningEffortConfig, ReasoningSummaryConfig } from '../protocol/types';
import { HistoryEntry } from '../protocol/events';
import { v4 as uuidv4 } from 'uuid';

/**
 * Turn context containing current session configuration
 */
export interface TurnContext {
  cwd: string;
  approval_policy: AskForApproval;
  sandbox_policy: SandboxPolicy;
  model: string;
  effort?: ReasoningEffortConfig;
  summary: ReasoningSummaryConfig;
}

/**
 * Session class managing conversation state
 */
export class Session {
  readonly conversationId: string;
  private history: HistoryEntry[] = [];
  private turnContext: TurnContext;
  private messageCount: number = 0;
  private currentTurnItems: InputItem[] = [];
  
  constructor() {
    this.conversationId = `conv_${uuidv4()}`;
    
    // Initialize with default turn context
    this.turnContext = {
      cwd: '/',
      approval_policy: 'on-request',
      sandbox_policy: { mode: 'workspace-write' },
      model: 'claude-3-sonnet',
      summary: { enabled: false },
    };
  }

  /**
   * Update turn context with new values
   */
  updateTurnContext(updates: Partial<TurnContext>): void {
    this.turnContext = {
      ...this.turnContext,
      ...updates,
    };
  }

  /**
   * Get current turn context
   */
  getTurnContext(): TurnContext {
    return { ...this.turnContext };
  }

  /**
   * Add a message to history
   */
  addToHistory(entry: HistoryEntry): void {
    this.history.push(entry);
    this.messageCount++;
  }

  /**
   * Get conversation history
   */
  getHistory(): HistoryEntry[] {
    return [...this.history];
  }

  /**
   * Get history entry by offset
   * @param offset Negative offset from end of history
   */
  getHistoryEntry(offset: number): HistoryEntry | undefined {
    if (offset >= 0 || Math.abs(offset) > this.history.length) {
      return undefined;
    }
    return this.history[this.history.length + offset];
  }

  /**
   * Clear conversation history
   */
  clearHistory(): void {
    this.history = [];
    this.messageCount = 0;
  }

  /**
   * Get current message count
   */
  getMessageCount(): number {
    return this.messageCount;
  }

  /**
   * Set current turn input items
   */
  setCurrentTurnItems(items: InputItem[]): void {
    this.currentTurnItems = items;
  }

  /**
   * Get current turn input items
   */
  getCurrentTurnItems(): InputItem[] {
    return [...this.currentTurnItems];
  }

  /**
   * Clear current turn items
   */
  clearCurrentTurn(): void {
    this.currentTurnItems = [];
  }

  /**
   * Get session metadata
   */
  getMetadata(): {
    conversationId: string;
    messageCount: number;
    startTime: number;
    currentModel: string;
  } {
    return {
      conversationId: this.conversationId,
      messageCount: this.messageCount,
      startTime: this.history[0]?.timestamp || Date.now(),
      currentModel: this.turnContext.model,
    };
  }

  /**
   * Export session for persistence
   */
  export(): {
    conversationId: string;
    history: HistoryEntry[];
    turnContext: TurnContext;
    messageCount: number;
  } {
    return {
      conversationId: this.conversationId,
      history: [...this.history],
      turnContext: { ...this.turnContext },
      messageCount: this.messageCount,
    };
  }

  /**
   * Import session from persistence
   */
  static import(data: {
    conversationId: string;
    history: HistoryEntry[];
    turnContext: TurnContext;
    messageCount: number;
  }): Session {
    const session = new Session();
    Object.assign(session, {
      conversationId: data.conversationId,
      history: [...data.history],
      turnContext: { ...data.turnContext },
      messageCount: data.messageCount,
    });
    return session;
  }

  /**
   * Check if session is empty
   */
  isEmpty(): boolean {
    return this.history.length === 0;
  }

  /**
   * Get last message from history
   */
  getLastMessage(): HistoryEntry | undefined {
    return this.history[this.history.length - 1];
  }

  /**
   * Get messages by type
   */
  getMessagesByType(type: 'user' | 'agent'): HistoryEntry[] {
    return this.history.filter(entry => entry.type === type);
  }
}
