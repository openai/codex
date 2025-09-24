/**
 * Message router for Chrome extension communication
 * Handles message passing between background, content scripts, and side panel
 */

import type { Submission, Event } from '../protocol/types';
import type { EventMsg } from '../protocol/events';

/**
 * Message types for Chrome extension communication
 */
export enum MessageType {
  // Core protocol messages
  SUBMISSION = 'SUBMISSION',
  EVENT = 'EVENT',

  // Connection management
  CONNECT = 'CONNECT',
  DISCONNECT = 'DISCONNECT',
  PING = 'PING',
  PONG = 'PONG',

  // State queries
  GET_STATE = 'GET_STATE',
  STATE_UPDATE = 'STATE_UPDATE',

  // Tab operations
  TAB_COMMAND = 'TAB_COMMAND',
  TAB_RESULT = 'TAB_RESULT',

  // Storage operations
  STORAGE_GET = 'STORAGE_GET',
  STORAGE_SET = 'STORAGE_SET',
  STORAGE_RESULT = 'STORAGE_RESULT',

  // Model client messages
  MODEL_REQUEST = 'MODEL_REQUEST',
  MODEL_RESPONSE = 'MODEL_RESPONSE',
  MODEL_ERROR = 'MODEL_ERROR',

  // Tool execution messages
  TOOL_EXECUTE = 'TOOL_EXECUTE',
  TOOL_RESULT = 'TOOL_RESULT',
  TOOL_ERROR = 'TOOL_ERROR',

  // Approval messages
  APPROVAL_REQUEST = 'APPROVAL_REQUEST',
  APPROVAL_RESPONSE = 'APPROVAL_RESPONSE',
  APPROVAL_TIMEOUT = 'APPROVAL_TIMEOUT',

  // Diff tracking messages
  DIFF_GENERATED = 'DIFF_GENERATED',
  DIFF_APPLIED = 'DIFF_APPLIED',
  DIFF_REJECTED = 'DIFF_REJECTED',
  DIFF_UPDATED = 'DIFF_UPDATED',

  // Turn management messages
  TURN_STARTED = 'TURN_STARTED',
  TURN_COMPLETED = 'TURN_COMPLETED',
  TURN_ABORTED = 'TURN_ABORTED',
}

/**
 * Chrome extension message format
 */
export interface ExtensionMessage {
  type: MessageType;
  payload?: any;
  id?: string;
  source?: 'background' | 'content' | 'sidepanel' | 'popup';
  tabId?: number;
  timestamp?: number;
}

/**
 * Response format for messages
 */
export interface MessageResponse {
  success: boolean;
  data?: any;
  error?: string;
}

/**
 * Message router class
 */
export class MessageRouter {
  private handlers: Map<MessageType, Set<MessageHandler>> = new Map();
  private pendingRequests: Map<string, PendingRequest> = new Map();
  private messageIdCounter: number = 0;
  private source: ExtensionMessage['source'];
  private connected: boolean = false;

  constructor(source: ExtensionMessage['source']) {
    this.source = source;
    this.setupMessageListener();
  }

  /**
   * Setup Chrome runtime message listener
   */
  private setupMessageListener(): void {
    if (typeof chrome !== 'undefined' && chrome.runtime) {
      chrome.runtime.onMessage.addListener(
        (message: ExtensionMessage, sender, sendResponse) => {
          this.handleMessage(message, sender, sendResponse);
          // Return true to indicate async response
          return true;
        }
      );

      // Setup connection listeners for persistent connections
      chrome.runtime.onConnect.addListener((port) => {
        this.handleConnection(port);
      });
    }
  }

  /**
   * Handle incoming message
   */
  private async handleMessage(
    message: ExtensionMessage,
    sender: chrome.runtime.MessageSender,
    sendResponse: (response: MessageResponse) => void
  ): Promise<void> {
    try {
      // Add sender info to message
      message.tabId = sender.tab?.id;
      message.timestamp = Date.now();

      // Handle response messages
      if (message.id && this.pendingRequests.has(message.id)) {
        const request = this.pendingRequests.get(message.id)!;
        this.pendingRequests.delete(message.id);
        request.resolve(message.payload);
        sendResponse({ success: true });
        return;
      }

      // Process message through handlers
      const handlers = this.handlers.get(message.type);
      if (handlers && handlers.size > 0) {
        const responses: any[] = [];
        
        for (const handler of handlers) {
          try {
            const result = await handler(message, sender);
            if (result !== undefined) {
              responses.push(result);
            }
          } catch (error) {
            console.error(`Handler error for ${message.type}:`, error);
          }
        }

        // Send first response back
        if (responses.length > 0) {
          sendResponse({ success: true, data: responses[0] });
        } else {
          sendResponse({ success: true });
        }
      } else {
        sendResponse({ 
          success: false, 
          error: `No handler for message type: ${message.type}` 
        });
      }
    } catch (error) {
      sendResponse({
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error',
      });
    }
  }

  /**
   * Handle persistent connection
   */
  private handleConnection(port: chrome.runtime.Port): void {
    console.log(`Connection established: ${port.name}`);
    
    port.onMessage.addListener((message) => {
      this.handlePortMessage(port, message);
    });

    port.onDisconnect.addListener(() => {
      console.log(`Connection closed: ${port.name}`);
      this.connected = false;
    });

    this.connected = true;
  }

  /**
   * Handle message from persistent port
   */
  private async handlePortMessage(
    port: chrome.runtime.Port,
    message: ExtensionMessage
  ): Promise<void> {
    // Process through regular handlers
    const handlers = this.handlers.get(message.type);
    if (handlers) {
      for (const handler of handlers) {
        try {
          const result = await handler(message, { tab: { id: port.sender?.tab?.id } } as any);
          if (result !== undefined) {
            port.postMessage({
              type: message.type,
              payload: result,
              id: message.id,
            });
          }
        } catch (error) {
          console.error(`Port handler error for ${message.type}:`, error);
        }
      }
    }
  }

  /**
   * Register message handler
   */
  on(
    type: MessageType,
    handler: MessageHandler
  ): () => void {
    if (!this.handlers.has(type)) {
      this.handlers.set(type, new Set());
    }
    
    this.handlers.get(type)!.add(handler);

    // Return unsubscribe function
    return () => {
      this.handlers.get(type)?.delete(handler);
    };
  }

  /**
   * Send message to extension
   */
  async send(
    type: MessageType,
    payload?: any,
    tabId?: number
  ): Promise<any> {
    const messageId = `msg_${++this.messageIdCounter}`;
    const message: ExtensionMessage = {
      type,
      payload,
      id: messageId,
      source: this.source,
      timestamp: Date.now(),
    };

    return new Promise((resolve, reject) => {
      // Store pending request
      this.pendingRequests.set(messageId, {
        resolve,
        reject,
        timestamp: Date.now(),
      });

      // Set timeout for response
      setTimeout(() => {
        if (this.pendingRequests.has(messageId)) {
          this.pendingRequests.delete(messageId);
          reject(new Error('Message timeout'));
        }
      }, 30000); // 30 second timeout

      // Send message
      if (tabId) {
        // Send to specific tab
        chrome.tabs.sendMessage(tabId, message, (response) => {
          if (chrome.runtime.lastError) {
            this.pendingRequests.delete(messageId);
            reject(chrome.runtime.lastError);
          } else if (response?.success === false) {
            this.pendingRequests.delete(messageId);
            reject(new Error(response.error || 'Message failed'));
          } else {
            this.pendingRequests.delete(messageId);
            resolve(response?.data);
          }
        });
      } else {
        // Send to extension runtime
        chrome.runtime.sendMessage(message, (response) => {
          if (chrome.runtime.lastError) {
            this.pendingRequests.delete(messageId);
            reject(chrome.runtime.lastError);
          } else if (response?.success === false) {
            this.pendingRequests.delete(messageId);
            reject(new Error(response.error || 'Message failed'));
          } else {
            this.pendingRequests.delete(messageId);
            resolve(response?.data);
          }
        });
      }
    });
  }

  /**
   * Broadcast message to all tabs
   */
  async broadcast(
    type: MessageType,
    payload?: any
  ): Promise<void> {
    const tabs = await chrome.tabs.query({});
    const promises = tabs.map(tab => {
      if (tab.id) {
        return this.send(type, payload, tab.id).catch(() => {
          // Ignore errors for individual tabs
        });
      }
    });
    
    await Promise.all(promises);
  }

  /**
   * Send submission to agent
   */
  async sendSubmission(submission: Submission): Promise<void> {
    await this.send(MessageType.SUBMISSION, submission);
  }

  /**
   * Send event from agent
   */
  async sendEvent(event: Event): Promise<void> {
    await this.send(MessageType.EVENT, event);
  }

  /**
   * Request current state
   */
  async getState(): Promise<any> {
    return this.send(MessageType.GET_STATE);
  }

  /**
   * Send state update
   */
  async updateState(state: any): Promise<void> {
    await this.send(MessageType.STATE_UPDATE, state);
  }

  /**
   * Execute tab command
   */
  async executeTabCommand(
    tabId: number,
    command: string,
    args?: any
  ): Promise<any> {
    return this.send(
      MessageType.TAB_COMMAND,
      { command, args },
      tabId
    );
  }

  /**
   * Storage operations
   */
  async storageGet(key: string): Promise<any> {
    return this.send(MessageType.STORAGE_GET, { key });
  }

  async storageSet(key: string, value: any): Promise<void> {
    await this.send(MessageType.STORAGE_SET, { key, value });
  }

  /**
   * Model client operations
   */
  async sendModelRequest(config: any, prompt: string): Promise<any> {
    return this.send(MessageType.MODEL_REQUEST, { config, prompt });
  }

  async sendModelResponse(response: any): Promise<void> {
    await this.send(MessageType.MODEL_RESPONSE, response);
  }

  async sendModelError(error: string): Promise<void> {
    await this.send(MessageType.MODEL_ERROR, { error });
  }

  /**
   * Tool execution operations
   */
  async executeToolMessage(toolName: string, args: any): Promise<any> {
    return this.send(MessageType.TOOL_EXECUTE, { toolName, args });
  }

  async sendToolResult(result: any): Promise<void> {
    await this.send(MessageType.TOOL_RESULT, result);
  }

  async sendToolError(error: string): Promise<void> {
    await this.send(MessageType.TOOL_ERROR, { error });
  }

  /**
   * Approval operations
   */
  async requestApproval(approvalId: string, type: string, details: any): Promise<any> {
    return this.send(MessageType.APPROVAL_REQUEST, { approvalId, type, details });
  }

  async sendApprovalResponse(approvalId: string, approved: boolean): Promise<void> {
    await this.send(MessageType.APPROVAL_RESPONSE, { approvalId, approved });
  }

  async sendApprovalTimeout(approvalId: string): Promise<void> {
    await this.send(MessageType.APPROVAL_TIMEOUT, { approvalId });
  }

  /**
   * Diff tracking operations
   */
  async sendDiffGenerated(diffId: string, path: string, content: any): Promise<void> {
    await this.send(MessageType.DIFF_GENERATED, { diffId, path, content });
  }

  async sendDiffApplied(diffId: string): Promise<void> {
    await this.send(MessageType.DIFF_APPLIED, { diffId });
  }

  async sendDiffRejected(diffId: string): Promise<void> {
    await this.send(MessageType.DIFF_REJECTED, { diffId });
  }

  async sendDiffUpdated(diffId: string, content: any): Promise<void> {
    await this.send(MessageType.DIFF_UPDATED, { diffId, content });
  }

  /**
   * Turn management operations
   */
  async sendTurnStarted(turnId: string): Promise<void> {
    await this.send(MessageType.TURN_STARTED, { turnId });
  }

  async sendTurnCompleted(turnId: string, result: any): Promise<void> {
    await this.send(MessageType.TURN_COMPLETED, { turnId, result });
  }

  async sendTurnAborted(turnId: string, reason: string): Promise<void> {
    await this.send(MessageType.TURN_ABORTED, { turnId, reason });
  }

  /**
   * Check if connected
   */
  isConnected(): boolean {
    return this.connected;
  }

  /**
   * Clean up pending requests
   */
  cleanup(): void {
    // Reject all pending requests
    for (const [id, request] of this.pendingRequests) {
      request.reject(new Error('Router cleanup'));
    }
    this.pendingRequests.clear();
    
    // Clear handlers
    this.handlers.clear();
  }
}

/**
 * Message handler type
 */
type MessageHandler = (
  message: ExtensionMessage,
  sender: chrome.runtime.MessageSender
) => Promise<any> | any;

/**
 * Pending request tracker
 */
interface PendingRequest {
  resolve: (value: any) => void;
  reject: (error: Error) => void;
  timestamp: number;
}

/**
 * Create router for current context
 */
export function createRouter(): MessageRouter {
  // Determine source based on context
  let source: ExtensionMessage['source'] = 'background';
  
  if (typeof chrome !== 'undefined') {
    if (chrome.sidePanel) {
      source = 'sidepanel';
    } else if (window.location.protocol === 'chrome-extension:') {
      // Could be popup or background
      if (document.querySelector('body')) {
        source = 'popup';
      }
    } else {
      source = 'content';
    }
  }

  return new MessageRouter(source);
}
