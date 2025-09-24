/**
 * Contract tests for TaskRunner
 * Tests task execution lifecycle and event emission
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { EventCollector, createMockSubmission, createMockUserTurn, waitFor, createDeferred } from '../utils/test-helpers';
import { Submission, Event, EventMsg } from '../../protocol/types';

// Define TaskRunner contract interface
interface TaskResult {
  success: boolean;
  finalMessage?: string;
  turns: Turn[];
  totalUsage: TokenUsage;
  changes: ChangeRecord[];
  error?: TaskError;
}

interface Turn {
  input: any[];
  output: any[];
  toolCalls: ToolCall[];
  tokenUsage: TokenUsage;
}

interface TokenUsage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}

interface ChangeRecord {
  id: string;
  type: 'dom' | 'storage' | 'navigation';
  before?: any;
  after?: any;
  timestamp: number;
}

interface TaskError {
  code: string;
  message: string;
  details?: any;
}

interface TaskStatus {
  isRunning: boolean;
  currentTurn?: number;
  phase?: 'initializing' | 'prompting' | 'tool_execution' | 'finalizing';
  lastActivity?: number;
}

interface TaskRunner {
  execute(submission: Submission): Promise<TaskResult>;
  interrupt(): Promise<void>;
  getStatus(): TaskStatus;
}

describe('TaskRunner Contract', () => {
  let eventCollector: EventCollector;

  beforeEach(() => {
    eventCollector = new EventCollector();
  });

  describe('Task Execution Request/Response', () => {
    it('should handle TaskExecutionRequest and return TaskResult', async () => {
      const submission = createMockSubmission(
        createMockUserTurn('Test task execution')
      );

      // Mock TaskRunner for contract testing
      const mockTaskRunner: TaskRunner = {
        async execute(sub: Submission): Promise<TaskResult> {
          // Emit TaskStarted event
          eventCollector.collect({
            id: 'evt_1',
            msg: {
              type: 'TaskStarted',
              data: {
                submission_id: sub.id,
                turn_type: 'user',
              },
            },
          });

          // Simulate task execution
          await new Promise(resolve => setTimeout(resolve, 10));

          // Emit TaskComplete event
          eventCollector.collect({
            id: 'evt_2',
            msg: {
              type: 'TaskComplete',
              data: { submission_id: sub.id },
            },
          });

          return {
            success: true,
            finalMessage: 'Task completed successfully',
            turns: [{
              input: [{ type: 'text', content: 'Test task execution' }],
              output: [{ type: 'text', content: 'Task result' }],
              toolCalls: [],
              tokenUsage: {
                promptTokens: 10,
                completionTokens: 5,
                totalTokens: 15,
              },
            }],
            totalUsage: {
              promptTokens: 10,
              completionTokens: 5,
              totalTokens: 15,
            },
            changes: [],
            error: undefined,
          };
        },
        async interrupt(): Promise<void> {
          // Handle interruption
        },
        getStatus(): TaskStatus {
          return {
            isRunning: false,
            currentTurn: 0,
            phase: 'initializing',
            lastActivity: Date.now(),
          };
        },
      };

      const result = await mockTaskRunner.execute(submission);

      // Verify result structure
      expect(result).toMatchObject({
        success: true,
        finalMessage: expect.any(String),
        turns: expect.arrayContaining([
          expect.objectContaining({
            input: expect.any(Array),
            output: expect.any(Array),
            toolCalls: expect.any(Array),
            tokenUsage: expect.objectContaining({
              promptTokens: expect.any(Number),
              completionTokens: expect.any(Number),
              totalTokens: expect.any(Number),
            }),
          }),
        ]),
        totalUsage: expect.objectContaining({
          promptTokens: expect.any(Number),
          completionTokens: expect.any(Number),
          totalTokens: expect.any(Number),
        }),
        changes: expect.any(Array),
      });

      // Verify events were emitted
      const events = eventCollector.getEvents();
      expect(events).toHaveLength(2);
      expect(events[0].msg.type).toBe('TaskStarted');
      expect(events[1].msg.type).toBe('TaskComplete');
    });

    it('should handle task failure', async () => {
      const submission = createMockSubmission(
        createMockUserTurn('Failing task')
      );

      const mockTaskRunner: TaskRunner = {
        async execute(sub: Submission): Promise<TaskResult> {
          eventCollector.collect({
            id: 'evt_1',
            msg: {
              type: 'TaskStarted',
              data: {
                submission_id: sub.id,
                turn_type: 'user',
              },
            },
          });

          // Simulate failure
          eventCollector.collect({
            id: 'evt_2',
            msg: {
              type: 'Error',
              data: {
                code: 'TASK_FAILED',
                message: 'Task execution failed',
                details: { reason: 'API error' },
              },
            },
          });

          return {
            success: false,
            turns: [],
            totalUsage: { promptTokens: 0, completionTokens: 0, totalTokens: 0 },
            changes: [],
            error: {
              code: 'TASK_FAILED',
              message: 'Task execution failed',
              details: { reason: 'API error' },
            },
          };
        },
        async interrupt(): Promise<void> {},
        getStatus(): TaskStatus {
          return { isRunning: false, phase: 'initializing', lastActivity: Date.now() };
        },
      };

      const result = await mockTaskRunner.execute(submission);

      expect(result.success).toBe(false);
      expect(result.error).toBeDefined();
      expect(result.error?.code).toBe('TASK_FAILED');

      const errorEvent = eventCollector.findByType('Error');
      expect(errorEvent).toBeDefined();
    });
  });

  describe('Task Cancellation', () => {
    it('should support task interruption', async () => {
      const deferred = createDeferred<TaskResult>();
      let isInterrupted = false;

      const mockTaskRunner: TaskRunner = {
        async execute(sub: Submission): Promise<TaskResult> {
          eventCollector.collect({
            id: 'evt_1',
            msg: {
              type: 'TaskStarted',
              data: {
                submission_id: sub.id,
                turn_type: 'user',
              },
            },
          });

          // Wait for interruption or completion
          return await Promise.race([
            deferred.promise,
            new Promise<TaskResult>(resolve => {
              const checkInterval = setInterval(() => {
                if (isInterrupted) {
                  clearInterval(checkInterval);
                  resolve({
                    success: false,
                    turns: [],
                    totalUsage: { promptTokens: 0, completionTokens: 0, totalTokens: 0 },
                    changes: [],
                    error: {
                      code: 'INTERRUPTED',
                      message: 'Task was interrupted by user',
                    },
                  });
                }
              }, 10);
            }),
          ]);
        },
        async interrupt(): Promise<void> {
          isInterrupted = true;
          eventCollector.collect({
            id: 'evt_abort',
            msg: {
              type: 'TurnAborted',
              data: {
                submission_id: 'sub_1',
                reason: 'user_interrupt',
              },
            },
          });
        },
        getStatus(): TaskStatus {
          return { isRunning: !isInterrupted, phase: 'prompting', lastActivity: Date.now() };
        },
      };

      const submission = createMockSubmission(createMockUserTurn('Long running task'));

      // Start task execution
      const executionPromise = mockTaskRunner.execute(submission);

      // Interrupt after a delay
      setTimeout(() => mockTaskRunner.interrupt(), 50);

      const result = await executionPromise;

      expect(result.success).toBe(false);
      expect(result.error?.code).toBe('INTERRUPTED');

      const abortEvent = eventCollector.findByType('TurnAborted');
      expect(abortEvent).toBeDefined();
      expect((abortEvent?.msg as any).data.reason).toBe('user_interrupt');
    });
  });

  describe('Progress Events', () => {
    it('should emit progress events during execution', async () => {
      const mockTaskRunner: TaskRunner = {
        async execute(sub: Submission): Promise<TaskResult> {
          // Emit various progress events
          const events: EventMsg[] = [
            {
              type: 'TaskStarted',
              data: { submission_id: sub.id, turn_type: 'user' },
            },
            {
              type: 'AgentMessage',
              data: { message: 'Processing your request...' },
            },
            {
              type: 'ExecCommandBegin',
              data: {
                session_id: 'session_1',
                command: 'browser.tabs.create',
                tab_id: 1,
              },
            },
            {
              type: 'ExecCommandEnd',
              data: {
                session_id: 'session_1',
                exit_code: 0,
                duration_ms: 100,
              },
            },
            {
              type: 'TaskComplete',
              data: { submission_id: sub.id },
            },
          ];

          for (const [index, msg] of events.entries()) {
            eventCollector.collect({
              id: `evt_${index + 1}`,
              msg,
            });
            await new Promise(resolve => setTimeout(resolve, 10));
          }

          return {
            success: true,
            finalMessage: 'Task completed with tool execution',
            turns: [{
              input: [],
              output: [],
              toolCalls: [{
                id: 'call_1',
                type: 'function',
                function: {
                  name: 'browser.tabs.create',
                  arguments: '{"url": "https://example.com"}',
                },
              }],
              tokenUsage: { promptTokens: 20, completionTokens: 10, totalTokens: 30 },
            }],
            totalUsage: { promptTokens: 20, completionTokens: 10, totalTokens: 30 },
            changes: [],
          };
        },
        async interrupt(): Promise<void> {},
        getStatus(): TaskStatus {
          return { isRunning: true, currentTurn: 1, phase: 'tool_execution', lastActivity: Date.now() };
        },
      };

      const submission = createMockSubmission(createMockUserTurn('Execute with progress'));
      const result = await mockTaskRunner.execute(submission);

      expect(result.success).toBe(true);

      const events = eventCollector.getEvents();
      expect(events.length).toBeGreaterThan(2);

      // Verify event sequence
      expect(events[0].msg.type).toBe('TaskStarted');
      expect(events[events.length - 1].msg.type).toBe('TaskComplete');

      // Verify tool execution events
      const execBegin = eventCollector.findByType('ExecCommandBegin');
      const execEnd = eventCollector.findByType('ExecCommandEnd');
      expect(execBegin).toBeDefined();
      expect(execEnd).toBeDefined();
    });
  });

  describe('Error Handling', () => {
    it('should handle errors gracefully', async () => {
      const mockTaskRunner: TaskRunner = {
        async execute(sub: Submission): Promise<TaskResult> {
          eventCollector.collect({
            id: 'evt_1',
            msg: {
              type: 'TaskStarted',
              data: { submission_id: sub.id, turn_type: 'user' },
            },
          });

          try {
            throw new Error('Simulated error');
          } catch (error: any) {
            eventCollector.collect({
              id: 'evt_2',
              msg: {
                type: 'Error',
                data: {
                  code: 'EXECUTION_ERROR',
                  message: error.message,
                  details: { stack: error.stack },
                },
              },
            });

            return {
              success: false,
              turns: [],
              totalUsage: { promptTokens: 0, completionTokens: 0, totalTokens: 0 },
              changes: [],
              error: {
                code: 'EXECUTION_ERROR',
                message: error.message,
                details: { stack: error.stack },
              },
            };
          }
        },
        async interrupt(): Promise<void> {},
        getStatus(): TaskStatus {
          return { isRunning: false, phase: 'initializing', lastActivity: Date.now() };
        },
      };

      const submission = createMockSubmission(createMockUserTurn('Error test'));
      const result = await mockTaskRunner.execute(submission);

      expect(result.success).toBe(false);
      expect(result.error).toBeDefined();
      expect(result.error?.code).toBe('EXECUTION_ERROR');
      expect(result.error?.message).toContain('Simulated error');

      const errorEvent = eventCollector.findByType('Error');
      expect(errorEvent).toBeDefined();
    });

    it('should validate task status during execution', () => {
      const mockTaskRunner: TaskRunner = {
        async execute(sub: Submission): Promise<TaskResult> {
          throw new Error('Not implemented');
        },
        async interrupt(): Promise<void> {},
        getStatus(): TaskStatus {
          return {
            isRunning: true,
            currentTurn: 2,
            phase: 'tool_execution',
            lastActivity: Date.now() - 1000,
          };
        },
      };

      const status = mockTaskRunner.getStatus();

      expect(status.isRunning).toBe(true);
      expect(status.currentTurn).toBe(2);
      expect(status.phase).toBe('tool_execution');
      expect(status.lastActivity).toBeLessThan(Date.now());
    });
  });
});