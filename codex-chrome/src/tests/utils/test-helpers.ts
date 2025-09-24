/**
 * Test helper utilities
 * Common functions for testing async operations and Chrome extension components
 */

import { expect } from 'vitest';
import { Event, EventMsg, Submission, Op } from '../../protocol/types';

/**
 * Wait for a condition to be true
 */
export async function waitFor(
  condition: () => boolean | Promise<boolean>,
  timeout = 5000,
  interval = 100
): Promise<void> {
  const startTime = Date.now();

  while (Date.now() - startTime < timeout) {
    if (await condition()) {
      return;
    }
    await sleep(interval);
  }

  throw new Error(`Timeout waiting for condition after ${timeout}ms`);
}

/**
 * Sleep for specified milliseconds
 */
export function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Create a mock submission
 */
export function createMockSubmission(op: Op, id = 'sub_1'): Submission {
  return { id, op };
}

/**
 * Create a mock event
 */
export function createMockEvent(msg: EventMsg, id = 'evt_1'): Event {
  return { id, msg };
}

/**
 * Create a mock user input operation
 */
export function createMockUserInput(content: string): Op {
  return {
    type: 'UserInput',
    items: [{ type: 'text', content }],
  };
}

/**
 * Create a mock user turn operation
 */
export function createMockUserTurn(content: string, model = 'gpt-4'): Op {
  return {
    type: 'UserTurn',
    items: [{ type: 'text', content }],
    cwd: '/mock/path',
    approval_policy: 'OnChange',
    sandbox_policy: 'TabWrite',
    model,
    summary: { enabled: false },
  };
}

/**
 * Mock async iterator for testing streaming responses
 */
export async function* createMockStream<T>(items: T[], delay = 10): AsyncGenerator<T> {
  for (const item of items) {
    await sleep(delay);
    yield item;
  }
}

/**
 * Collect all items from an async iterator
 */
export async function collectStream<T>(iterator: AsyncIterable<T>): Promise<T[]> {
  const items: T[] = [];
  for await (const item of iterator) {
    items.push(item);
  }
  return items;
}

/**
 * Create a deferred promise for testing async flows
 */
export function createDeferred<T>(): {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (error: any) => void;
} {
  let resolve: (value: T) => void;
  let reject: (error: any) => void;

  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve: resolve!, reject: reject! };
}

/**
 * Mock fetch for API testing
 */
export function createMockFetch(responses: Map<string, any>) {
  return vi.fn(async (url: string, options?: RequestInit) => {
    const response = responses.get(url);
    if (!response) {
      throw new Error(`No mock response for ${url}`);
    }

    return {
      ok: true,
      status: 200,
      json: async () => response,
      text: async () => JSON.stringify(response),
      headers: new Headers({
        'content-type': 'application/json',
      }),
    };
  });
}

/**
 * Event collector for testing event emission
 */
export class EventCollector {
  private events: Event[] = [];

  collect(event: Event) {
    this.events.push(event);
  }

  getEvents(): Event[] {
    return [...this.events];
  }

  getLastEvent(): Event | undefined {
    return this.events[this.events.length - 1];
  }

  clear() {
    this.events = [];
  }

  findByType(type: string): Event | undefined {
    return this.events.find(e => e.msg.type === type);
  }

  filterByType(type: string): Event[] {
    return this.events.filter(e => e.msg.type === type);
  }
}

/**
 * Mock tool result
 */
export function createMockToolResult(success = true, data?: any, error?: string) {
  return {
    success,
    data,
    error,
  };
}

/**
 * Assert that a promise rejects with a specific error
 */
export async function assertRejects(
  promise: Promise<any>,
  expectedError?: string | RegExp
): Promise<void> {
  try {
    await promise;
    throw new Error('Expected promise to reject but it resolved');
  } catch (error: any) {
    if (expectedError) {
      if (typeof expectedError === 'string') {
        expect(error.message).toContain(expectedError);
      } else {
        expect(error.message).toMatch(expectedError);
      }
    }
  }
}