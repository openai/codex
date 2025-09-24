/**
 * Contract tests for TurnManager
 * Tests turn-based conversation management and state tracking
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { EventCollector, createMockSubmission, createMockUserTurn, waitFor, createDeferred } from '../utils/test-helpers';
import { Submission, Event, EventMsg } from '../../protocol/types';

// Define TurnManager contract interfaces
interface TurnRequest {
  sessionId: string;
  submission: Submission;
  context: TurnContext;
  retryCount?: number;
}

interface TurnResponse {
  turnId: string;
  sessionId: string;
  success: boolean;
  messages: TurnMessage[];
  toolCalls: ToolCall[];
  tokenUsage: TokenUsage;
  context: TurnContext;
  error?: TurnError;
}

interface TurnContext {
  conversationId: string;
  turnNumber: number;
  model: string;
  cwd: string;
  approvalPolicy: string;
  sandboxPolicy: string;
  temperature?: number;
  maxTokens?: number;
}

interface TurnMessage {
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string;
  timestamp: number;
  metadata?: any;
}

interface ToolCall {
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string;
  };
}

interface TokenUsage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}

interface TurnError {
  code: string;
  message: string;
  retryable: boolean;
  details?: any;
}

interface ConversationState {
  id: string;
  turnCount: number;
  totalTokenUsage: TokenUsage;
  isActive: boolean;
  lastActivity: number;
  context: TurnContext;
}

interface TurnManager {
  executeTurn(request: TurnRequest): Promise<TurnResponse>;
  retryTurn(turnId: string, newContext?: Partial<TurnContext>): Promise<TurnResponse>;
  getConversationState(sessionId: string): ConversationState | null;
  updateTurnContext(sessionId: string, updates: Partial<TurnContext>): Promise<void>;
  abortTurn(turnId: string): Promise<void>;
}

describe('TurnManager Contract', () => {
  let eventCollector: EventCollector;

  beforeEach(() => {
    eventCollector = new EventCollector();
  });

  describe('Turn Execution Request/Response', () => {
    it('should handle TurnRequest and return TurnResponse', async () => {
      const mockTurnManager: TurnManager = {
        async executeTurn(request: TurnRequest): Promise<TurnResponse> {
          // Emit turn started event
          eventCollector.collect({
            id: 'evt_1',
            msg: {
              type: 'TurnStarted',
              data: {
                session_id: request.sessionId,
                turn_id: 'turn_1',
                turn_number: request.context.turnNumber,
              },
            },
          });

          // Simulate processing delay
          await new Promise(resolve => setTimeout(resolve, 10));

          // Emit completion event
          eventCollector.collect({
            id: 'evt_2',
            msg: {
              type: 'TurnComplete',
              data: {
                session_id: request.sessionId,
                turn_id: 'turn_1',
                success: true,
              },
            },
          });

          return {
            turnId: 'turn_1',
            sessionId: request.sessionId,
            success: true,
            messages: [
              {
                role: 'user',
                content: 'Test input',
                timestamp: Date.now(),
              },
              {
                role: 'assistant',
                content: 'Test response',
                timestamp: Date.now(),
              },
            ],
            toolCalls: [],
            tokenUsage: {
              promptTokens: 15,
              completionTokens: 10,
              totalTokens: 25,
            },
            context: {
              ...request.context,
              turnNumber: request.context.turnNumber + 1,
            },
          };
        },
        async retryTurn(turnId: string): Promise<TurnResponse> {
          throw new Error('Not implemented');
        },
        getConversationState(sessionId: string): ConversationState | null {
          return null;
        },
        async updateTurnContext(): Promise<void> {},
        async abortTurn(): Promise<void> {},
      };

      const request: TurnRequest = {
        sessionId: 'session_1',
        submission: createMockSubmission(
          createMockUserTurn('Test turn execution')
        ),
        context: {
          conversationId: 'conv_1',
          turnNumber: 1,
          model: 'gpt-4',
          cwd: '/home/user',
          approvalPolicy: 'on-request',
          sandboxPolicy: 'read-only',
        },
      };

      const response = await mockTurnManager.executeTurn(request);

      // Verify response structure
      expect(response).toMatchObject({
        turnId: expect.any(String),
        sessionId: 'session_1',
        success: true,
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: expect.stringMatching(/^(user|assistant|system|tool)$/),
            content: expect.any(String),
            timestamp: expect.any(Number),
          }),
        ]),
        toolCalls: expect.any(Array),
        tokenUsage: expect.objectContaining({
          promptTokens: expect.any(Number),
          completionTokens: expect.any(Number),
          totalTokens: expect.any(Number),
        }),
        context: expect.objectContaining({
          conversationId: 'conv_1',
          turnNumber: expect.any(Number),
          model: expect.any(String),
        }),
      });

      // Verify events were emitted
      const events = eventCollector.getEvents();
      expect(events).toHaveLength(2);
      expect(events[0].msg.type).toBe('TurnStarted');
      expect(events[1].msg.type).toBe('TurnComplete');
    });

    it('should handle turn failure gracefully', async () => {
      const mockTurnManager: TurnManager = {
        async executeTurn(request: TurnRequest): Promise<TurnResponse> {
          eventCollector.collect({
            id: 'evt_1',
            msg: {
              type: 'TurnStarted',
              data: {
                session_id: request.sessionId,
                turn_id: 'turn_fail',
              },
            },
          });

          eventCollector.collect({
            id: 'evt_2',
            msg: {
              type: 'Error',
              data: {
                code: 'TURN_FAILED',
                message: 'Turn execution failed',
                retryable: true,
              },
            },
          });

          return {
            turnId: 'turn_fail',
            sessionId: request.sessionId,
            success: false,
            messages: [],
            toolCalls: [],
            tokenUsage: {
              promptTokens: 0,
              completionTokens: 0,
              totalTokens: 0,
            },
            context: request.context,
            error: {
              code: 'TURN_FAILED',
              message: 'Turn execution failed',
              retryable: true,
            },
          };
        },
        async retryTurn(): Promise<TurnResponse> {
          throw new Error('Not implemented');
        },
        getConversationState(): ConversationState | null {
          return null;
        },
        async updateTurnContext(): Promise<void> {},
        async abortTurn(): Promise<void> {},
      };

      const request: TurnRequest = {
        sessionId: 'session_1',
        submission: createMockSubmission(createMockUserTurn('Failing turn')),
        context: {
          conversationId: 'conv_1',
          turnNumber: 1,
          model: 'gpt-4',
          cwd: '/home/user',
          approvalPolicy: 'on-request',
          sandboxPolicy: 'read-only',
        },
      };

      const response = await mockTurnManager.executeTurn(request);

      expect(response.success).toBe(false);
      expect(response.error).toBeDefined();
      expect(response.error?.retryable).toBe(true);

      const errorEvent = eventCollector.findByType('Error');
      expect(errorEvent).toBeDefined();
    });
  });

  describe('Conversation State Management', () => {
    it('should track conversation state', () => {
      const mockConversationState: ConversationState = {
        id: 'conv_1',
        turnCount: 3,
        totalTokenUsage: {
          promptTokens: 100,
          completionTokens: 75,
          totalTokens: 175,
        },
        isActive: true,
        lastActivity: Date.now(),
        context: {
          conversationId: 'conv_1',
          turnNumber: 3,
          model: 'gpt-4',
          cwd: '/home/user',
          approvalPolicy: 'on-request',
          sandboxPolicy: 'read-only',
        },
      };

      const mockTurnManager: TurnManager = {
        async executeTurn(): Promise<TurnResponse> {
          throw new Error('Not implemented');
        },
        async retryTurn(): Promise<TurnResponse> {
          throw new Error('Not implemented');
        },
        getConversationState(sessionId: string): ConversationState | null {
          return sessionId === 'session_1' ? mockConversationState : null;
        },
        async updateTurnContext(): Promise<void> {},
        async abortTurn(): Promise<void> {},
      };

      const state = mockTurnManager.getConversationState('session_1');

      expect(state).toMatchObject({
        id: 'conv_1',
        turnCount: 3,
        totalTokenUsage: expect.objectContaining({
          promptTokens: expect.any(Number),
          completionTokens: expect.any(Number),
          totalTokens: expect.any(Number),
        }),
        isActive: true,
        lastActivity: expect.any(Number),
        context: expect.objectContaining({
          conversationId: 'conv_1',
          turnNumber: 3,
        }),
      });

      const nonExistentState = mockTurnManager.getConversationState('nonexistent');
      expect(nonExistentState).toBeNull();
    });

    it('should support context updates', async () => {
      let currentContext: TurnContext = {
        conversationId: 'conv_1',
        turnNumber: 1,
        model: 'gpt-4',
        cwd: '/home/user',
        approvalPolicy: 'on-request',
        sandboxPolicy: 'read-only',
      };

      const mockTurnManager: TurnManager = {
        async executeTurn(): Promise<TurnResponse> {
          throw new Error('Not implemented');
        },
        async retryTurn(): Promise<TurnResponse> {
          throw new Error('Not implemented');
        },
        getConversationState(sessionId: string): ConversationState | null {
          return {
            id: 'conv_1',
            turnCount: 1,
            totalTokenUsage: { promptTokens: 0, completionTokens: 0, totalTokens: 0 },
            isActive: true,
            lastActivity: Date.now(),
            context: currentContext,
          };
        },
        async updateTurnContext(sessionId: string, updates: Partial<TurnContext>): Promise<void> {
          if (sessionId === 'session_1') {
            currentContext = { ...currentContext, ...updates };

            eventCollector.collect({
              id: 'evt_context_update',
              msg: {
                type: 'ContextUpdated',
                data: {
                  session_id: sessionId,
                  updates,
                },
              },
            });
          }
        },
        async abortTurn(): Promise<void> {},
      };

      const updates: Partial<TurnContext> = {
        model: 'claude-3-opus',
        temperature: 0.8,
        maxTokens: 1000,
      };

      await mockTurnManager.updateTurnContext('session_1', updates);

      const updatedState = mockTurnManager.getConversationState('session_1');
      expect(updatedState?.context.model).toBe('claude-3-opus');
      expect(updatedState?.context.temperature).toBe(0.8);
      expect(updatedState?.context.maxTokens).toBe(1000);

      const contextEvent = eventCollector.findByType('ContextUpdated');
      expect(contextEvent).toBeDefined();
    });
  });

  describe('Retry Logic', () => {
    it('should support turn retry with optional context updates', async () => {
      const mockTurnManager: TurnManager = {
        async executeTurn(): Promise<TurnResponse> {
          throw new Error('Not implemented');
        },
        async retryTurn(turnId: string, newContext?: Partial<TurnContext>): Promise<TurnResponse> {
          eventCollector.collect({
            id: 'evt_retry',
            msg: {
              type: 'TurnRetry',
              data: {
                turn_id: turnId,
                retry_count: 1,
                context_updates: newContext || {},
              },
            },
          });

          return {
            turnId: `${turnId}_retry`,
            sessionId: 'session_1',
            success: true,
            messages: [
              {
                role: 'assistant',
                content: 'Retry successful',
                timestamp: Date.now(),
              },
            ],
            toolCalls: [],
            tokenUsage: {
              promptTokens: 12,
              completionTokens: 8,
              totalTokens: 20,
            },
            context: {
              conversationId: 'conv_1',
              turnNumber: 2,
              model: newContext?.model || 'gpt-4',
              cwd: '/home/user',
              approvalPolicy: 'on-request',
              sandboxPolicy: 'read-only',
              ...newContext,
            },
          };
        },
        getConversationState(): ConversationState | null {
          return null;
        },
        async updateTurnContext(): Promise<void> {},
        async abortTurn(): Promise<void> {},
      };

      const retryContext = {
        model: 'gpt-3.5-turbo',
        temperature: 0.5,
      };

      const retryResponse = await mockTurnManager.retryTurn('turn_fail', retryContext);

      expect(retryResponse.success).toBe(true);
      expect(retryResponse.turnId).toBe('turn_fail_retry');
      expect(retryResponse.context.model).toBe('gpt-3.5-turbo');
      expect(retryResponse.context.temperature).toBe(0.5);

      const retryEvent = eventCollector.findByType('TurnRetry');
      expect(retryEvent).toBeDefined();
      expect((retryEvent?.msg as any).data.retry_count).toBe(1);
    });

    it('should handle retry with incremental backoff', async () => {
      let retryCount = 0;

      const mockTurnManager: TurnManager = {
        async executeTurn(): Promise<TurnResponse> {
          throw new Error('Not implemented');
        },
        async retryTurn(turnId: string): Promise<TurnResponse> {
          retryCount++;

          eventCollector.collect({
            id: `evt_retry_${retryCount}`,
            msg: {
              type: 'TurnRetry',
              data: {
                turn_id: turnId,
                retry_count: retryCount,
                delay_ms: retryCount * 1000,
              },
            },
          });

          if (retryCount < 3) {
            throw new Error('Retry failed');
          }

          return {
            turnId: `${turnId}_retry_${retryCount}`,
            sessionId: 'session_1',
            success: true,
            messages: [],
            toolCalls: [],
            tokenUsage: { promptTokens: 0, completionTokens: 0, totalTokens: 0 },
            context: {
              conversationId: 'conv_1',
              turnNumber: 1,
              model: 'gpt-4',
              cwd: '/home/user',
              approvalPolicy: 'on-request',
              sandboxPolicy: 'read-only',
            },
          };
        },
        getConversationState(): ConversationState | null {
          return null;
        },
        async updateTurnContext(): Promise<void> {},
        async abortTurn(): Promise<void> {},
      };

      // Simulate retry logic with exponential backoff
      let response: TurnResponse | null = null;
      let attempts = 0;

      while (attempts < 3 && !response) {
        try {
          response = await mockTurnManager.retryTurn('turn_backoff');
        } catch (error) {
          attempts++;
          if (attempts < 3) {
            await new Promise(resolve => setTimeout(resolve, 10));
          }
        }
      }

      expect(response).toBeDefined();
      expect(response?.success).toBe(true);
      expect(retryCount).toBe(3);

      const retryEvents = eventCollector.filterByType('TurnRetry');
      expect(retryEvents).toHaveLength(3);
    });
  });

  describe('Turn Abortion', () => {
    it('should support turn abortion', async () => {
      const deferred = createDeferred<TurnResponse>();
      let isAborted = false;

      const mockTurnManager: TurnManager = {
        async executeTurn(request: TurnRequest): Promise<TurnResponse> {
          eventCollector.collect({
            id: 'evt_started',
            msg: {
              type: 'TurnStarted',
              data: {
                session_id: request.sessionId,
                turn_id: 'turn_abort',
              },
            },
          });

          return new Promise((resolve) => {
            const checkInterval = setInterval(() => {
              if (isAborted) {
                clearInterval(checkInterval);
                resolve({
                  turnId: 'turn_abort',
                  sessionId: request.sessionId,
                  success: false,
                  messages: [],
                  toolCalls: [],
                  tokenUsage: { promptTokens: 0, completionTokens: 0, totalTokens: 0 },
                  context: request.context,
                  error: {
                    code: 'ABORTED',
                    message: 'Turn was aborted by user',
                    retryable: false,
                  },
                });
              }
            }, 10);
          });
        },
        async retryTurn(): Promise<TurnResponse> {
          throw new Error('Not implemented');
        },
        getConversationState(): ConversationState | null {
          return null;
        },
        async updateTurnContext(): Promise<void> {},
        async abortTurn(turnId: string): Promise<void> {
          isAborted = true;
          eventCollector.collect({
            id: 'evt_abort',
            msg: {
              type: 'TurnAborted',
              data: {
                turn_id: turnId,
                reason: 'user_request',
              },
            },
          });
        },
      };

      const request: TurnRequest = {
        sessionId: 'session_1',
        submission: createMockSubmission(createMockUserTurn('Long running turn')),
        context: {
          conversationId: 'conv_1',
          turnNumber: 1,
          model: 'gpt-4',
          cwd: '/home/user',
          approvalPolicy: 'on-request',
          sandboxPolicy: 'read-only',
        },
      };

      // Start execution
      const executionPromise = mockTurnManager.executeTurn(request);

      // Abort after delay
      setTimeout(() => mockTurnManager.abortTurn('turn_abort'), 50);

      const result = await executionPromise;

      expect(result.success).toBe(false);
      expect(result.error?.code).toBe('ABORTED');
      expect(result.error?.retryable).toBe(false);

      const abortEvent = eventCollector.findByType('TurnAborted');
      expect(abortEvent).toBeDefined();
      expect((abortEvent?.msg as any).data.reason).toBe('user_request');
    });
  });
});