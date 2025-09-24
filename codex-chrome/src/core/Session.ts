/**
 * Session management class - port of Session struct from codex.rs
 * Manages conversation state, turn context, and history
 */

import type { InputItem, AskForApproval, SandboxPolicy, ReasoningEffortConfig, ReasoningSummaryConfig, Event } from '../protocol/types';
import type { HistoryEntry, EventMsg } from '../protocol/events';
import { v4 as uuidv4 } from 'uuid';

/**
 * Tool definition interface (to avoid circular dependency with TurnManager)
 */
export interface ToolDefinition {
  type: 'function';
  function: {
    name: string;
    description: string;
    parameters?: any;
  };
}

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
  private pendingInput: InputItem[] = [];
  private eventEmitter: ((event: Event) => Promise<void>) | null = null;
  
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

  /**
   * Set event emitter for sending events to the queue
   */
  setEventEmitter(emitter: (event: Event) => Promise<void>): void {
    this.eventEmitter = emitter;
  }

  /**
   * Emit an event
   */
  async emitEvent(event: Event): Promise<void> {
    if (this.eventEmitter) {
      await this.eventEmitter(event);
    } else {
      console.warn('Event emitter not set, event dropped:', event);
    }
  }

  /**
   * Get session ID (conversation ID)
   */
  getSessionId(): string {
    return this.conversationId;
  }

  /**
   * Record input items in conversation
   */
  async recordInput(items: InputItem[]): Promise<void> {
    const timestamp = Date.now();

    for (const item of items) {
      let text = '';

      switch (item.type) {
        case 'text':
          text = item.text;
          break;
        case 'image':
          text = '[image]';
          break;
        case 'clipboard':
          text = item.content || '[clipboard]';
          break;
        case 'context':
          text = `[context: ${item.path || 'unknown'}]`;
          break;
        default:
          text = '[unknown input]';
      }

      this.addToHistory({
        timestamp,
        text,
        type: 'user',
      });
    }
  }

  /**
   * Record conversation items (messages, tool calls, etc.)
   */
  async recordConversationItems(items: any[]): Promise<void> {
    const timestamp = Date.now();

    for (const item of items) {
      if (item.role === 'assistant' || item.role === 'user') {
        const text = this.extractTextFromItem(item);
        if (text) {
          this.addToHistory({
            timestamp,
            text,
            type: item.role === 'assistant' ? 'agent' : 'user',
          });
        }
      }
    }
  }

  /**
   * Extract text content from conversation items
   */
  private extractTextFromItem(item: any): string {
    if (typeof item.content === 'string') {
      return item.content;
    }

    if (Array.isArray(item.content)) {
      return item.content
        .filter((c: any) => c.type === 'text')
        .map((c: any) => c.text)
        .join(' ');
    }

    return '';
  }

  /**
   * Get pending user input during turn execution
   */
  async getPendingInput(): Promise<any[]> {
    const pending = [...this.pendingInput];
    this.pendingInput = []; // Clear pending input
    return pending.map(item => this.convertInputToResponse(item));
  }

  /**
   * Add pending input (for interrupting turns)
   */
  addPendingInput(items: InputItem[]): void {
    this.pendingInput.push(...items);
  }

  /**
   * Convert input item to response format
   */
  private convertInputToResponse(item: InputItem): any {
    switch (item.type) {
      case 'text':
        return {
          role: 'user',
          content: [{ type: 'text', text: item.text }],
        };
      case 'image':
        return {
          role: 'user',
          content: [{ type: 'image', image_url: item.image_url }],
        };
      case 'clipboard':
        return {
          role: 'user',
          content: [{ type: 'text', text: item.content || '[clipboard]' }],
        };
      case 'context':
        return {
          role: 'user',
          content: [{ type: 'text', text: `[context: ${item.path || 'unknown'}]` }],
        };
      default:
        return {
          role: 'user',
          content: [{ type: 'text', text: '[unknown]' }],
        };
    }
  }

  /**
   * Build turn input with full conversation history
   */
  async buildTurnInputWithHistory(newItems: any[]): Promise<any[]> {
    const historyItems = this.history.map(entry => ({
      role: entry.type === 'user' ? 'user' : 'assistant',
      content: [{ type: 'text', text: entry.text }],
    }));

    return [...historyItems, ...newItems];
  }

  /**
   * Get MCP tools available to the session
   */
  async getMcpTools(): Promise<ToolDefinition[]> {
    // Placeholder for MCP tools integration
    // In a full implementation, this would connect to MCP servers
    return [];
  }

  /**
   * Execute an MCP tool
   */
  async executeMcpTool(toolName: string, parameters: any): Promise<any> {
    // Placeholder for MCP tool execution
    // In a full implementation, this would call the appropriate MCP server
    throw new Error(`MCP tool '${toolName}' not implemented`);
  }

  /**
   * Record turn context for rollout/history
   */
  async recordTurnContext(contextItem: any): Promise<void> {
    // In a full implementation, this would persist turn context
    console.log('Recording turn context:', contextItem);
  }

  /**
   * Compact conversation history to save tokens
   */
  async compact(): Promise<void> {
    // Simple compaction strategy: keep last 20 messages
    if (this.history.length > 20) {
      const keepCount = 20;
      const toRemove = this.history.length - keepCount;
      this.history.splice(0, toRemove);
      this.messageCount = this.history.length;
    }
  }

  /**
   * Build initial context for review mode
   */
  buildInitialContext(turnContext?: any): any[] {
    return [
      {
        role: 'system',
        content: [
          {
            type: 'text',
            text: `Working directory: ${turnContext?.cwd || '/'}`,
          },
        ],
      },
    ];
  }
}
