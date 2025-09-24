/**
 * Queue processing utilities for managing submission and event queues
 * Implements async queue patterns from codex.rs
 */

import { Submission, Event, Op } from '../protocol/types';
import { EventMsg } from '../protocol/events';

/**
 * Priority levels for queue items
 */
export enum QueuePriority {
  HIGH = 0,
  NORMAL = 1,
  LOW = 2,
}

/**
 * Queue item with priority
 */
export interface QueueItem<T> {
  data: T;
  priority: QueuePriority;
  timestamp: number;
}

/**
 * Async queue implementation with priority support
 */
export class PriorityQueue<T> {
  private items: QueueItem<T>[] = [];
  private processing: boolean = false;
  private maxSize: number;

  constructor(maxSize: number = 1000) {
    this.maxSize = maxSize;
  }

  /**
   * Add item to queue with priority
   */
  enqueue(data: T, priority: QueuePriority = QueuePriority.NORMAL): boolean {
    if (this.items.length >= this.maxSize) {
      return false;
    }

    const item: QueueItem<T> = {
      data,
      priority,
      timestamp: Date.now(),
    };

    // Insert based on priority
    const index = this.items.findIndex(
      (i) => i.priority > priority || 
      (i.priority === priority && i.timestamp > item.timestamp)
    );

    if (index === -1) {
      this.items.push(item);
    } else {
      this.items.splice(index, 0, item);
    }

    return true;
  }

  /**
   * Remove and return next item
   */
  dequeue(): T | null {
    const item = this.items.shift();
    return item ? item.data : null;
  }

  /**
   * Peek at next item without removing
   */
  peek(): T | null {
    return this.items[0]?.data || null;
  }

  /**
   * Get queue size
   */
  size(): number {
    return this.items.length;
  }

  /**
   * Check if queue is empty
   */
  isEmpty(): boolean {
    return this.items.length === 0;
  }

  /**
   * Clear all items
   */
  clear(): void {
    this.items = [];
  }

  /**
   * Set processing state
   */
  setProcessing(state: boolean): void {
    this.processing = state;
  }

  /**
   * Check if queue is processing
   */
  isProcessing(): boolean {
    return this.processing;
  }

  /**
   * Get all items (for debugging)
   */
  getItems(): T[] {
    return this.items.map(item => item.data);
  }
}

/**
 * Submission queue specialized for Op operations
 */
export class SubmissionQueue extends PriorityQueue<Submission> {
  /**
   * Add submission with automatic priority assignment
   */
  submit(submission: Submission): boolean {
    // Assign priority based on operation type
    const priority = this.getPriorityForOp(submission.op);
    return this.enqueue(submission, priority);
  }

  /**
   * Get priority for operation type
   */
  private getPriorityForOp(op: Op): QueuePriority {
    switch (op.type) {
      case 'Interrupt':
      case 'Shutdown':
        return QueuePriority.HIGH;
      
      case 'ExecApproval':
      case 'PatchApproval':
        return QueuePriority.HIGH;
      
      case 'UserTurn':
      case 'UserInput':
        return QueuePriority.NORMAL;
      
      default:
        return QueuePriority.LOW;
    }
  }

  /**
   * Cancel all pending submissions of a specific type
   */
  cancelByType(type: Op['type']): number {
    const initialSize = this.size();
    const items = this.getItems();
    this.clear();

    for (const item of items) {
      if (item.op.type !== type) {
        this.submit(item);
      }
    }

    return initialSize - this.size();
  }
}

/**
 * Event queue specialized for EventMsg
 */
export class EventQueue extends PriorityQueue<Event> {
  private listeners: Map<string, Set<(event: Event) => void>> = new Map();

  /**
   * Emit event to queue
   */
  emit(event: Event): boolean {
    // Notify listeners immediately
    this.notifyListeners(event.msg.type, event);
    
    // Add to queue for processing
    const priority = this.getPriorityForEvent(event.msg);
    return this.enqueue(event, priority);
  }

  /**
   * Get priority for event type
   */
  private getPriorityForEvent(msg: EventMsg): QueuePriority {
    switch (msg.type) {
      case 'Error':
      case 'TurnAborted':
      case 'ShutdownComplete':
        return QueuePriority.HIGH;
      
      case 'ExecApprovalRequest':
      case 'ApplyPatchApprovalRequest':
        return QueuePriority.HIGH;
      
      case 'TaskStarted':
      case 'TaskComplete':
      case 'AgentMessage':
        return QueuePriority.NORMAL;
      
      default:
        return QueuePriority.LOW;
    }
  }

  /**
   * Subscribe to specific event type
   */
  on(eventType: string, callback: (event: Event) => void): () => void {
    if (!this.listeners.has(eventType)) {
      this.listeners.set(eventType, new Set());
    }
    
    this.listeners.get(eventType)!.add(callback);

    // Return unsubscribe function
    return () => {
      this.listeners.get(eventType)?.delete(callback);
    };
  }

  /**
   * Notify listeners of event
   */
  private notifyListeners(eventType: string, event: Event): void {
    const listeners = this.listeners.get(eventType);
    if (listeners) {
      listeners.forEach(callback => {
        try {
          callback(event);
        } catch (error) {
          console.error(`Error in event listener for ${eventType}:`, error);
        }
      });
    }
  }

  /**
   * Get events of specific type
   */
  getEventsByType(type: string): Event[] {
    return this.getItems().filter(event => event.msg.type === type);
  }
}

/**
 * Queue processor for handling submissions
 */
export class QueueProcessor {
  private submissionQueue: SubmissionQueue;
  private eventQueue: EventQueue;
  private processingInterval?: NodeJS.Timeout;
  private batchSize: number;

  constructor(
    submissionQueue: SubmissionQueue,
    eventQueue: EventQueue,
    batchSize: number = 1
  ) {
    this.submissionQueue = submissionQueue;
    this.eventQueue = eventQueue;
    this.batchSize = batchSize;
  }

  /**
   * Start processing queues
   */
  start(intervalMs: number = 10): void {
    if (this.processingInterval) {
      return;
    }

    this.processingInterval = setInterval(() => {
      this.processTick();
    }, intervalMs);
  }

  /**
   * Stop processing queues
   */
  stop(): void {
    if (this.processingInterval) {
      clearInterval(this.processingInterval);
      this.processingInterval = undefined;
    }
  }

  /**
   * Process one tick of the queue
   */
  private async processTick(): Promise<void> {
    // Skip if already processing
    if (this.submissionQueue.isProcessing()) {
      return;
    }

    // Process batch of submissions
    const submissions: Submission[] = [];
    for (let i = 0; i < this.batchSize && !this.submissionQueue.isEmpty(); i++) {
      const submission = this.submissionQueue.dequeue();
      if (submission) {
        submissions.push(submission);
      }
    }

    if (submissions.length > 0) {
      this.submissionQueue.setProcessing(true);
      // Processing will be handled by the agent
      // Just mark as not processing for next tick
      this.submissionQueue.setProcessing(false);
    }
  }

  /**
   * Get queue statistics
   */
  getStats(): {
    submissionQueueSize: number;
    eventQueueSize: number;
    isProcessing: boolean;
  } {
    return {
      submissionQueueSize: this.submissionQueue.size(),
      eventQueueSize: this.eventQueue.size(),
      isProcessing: this.submissionQueue.isProcessing(),
    };
  }
}
