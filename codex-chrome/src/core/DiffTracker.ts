/**
 * DiffTracker - Tracks DOM changes, storage changes, and provides rollback functionality
 * Based on contract from diff-tracker.test.ts
 */

import type { Event } from '../protocol/types';

export interface AddChangeRequest {
  changeId: string;
  type: 'dom' | 'storage' | 'navigation' | 'network' | 'file';
  operation: 'create' | 'update' | 'delete' | 'navigate';
  target: ChangeTarget;
  before?: any;
  after?: any;
  metadata?: ChangeMetadata;
}

export interface GetChangesRequest {
  sessionId?: string;
  turnId?: string;
  type?: string;
  since?: number;
  until?: number;
  limit?: number;
  includeRolledBack?: boolean;
}

export interface ChangeTarget {
  type: 'element' | 'storage_key' | 'url' | 'file_path' | 'network_request';
  selector?: string;
  storageKey?: string;
  storageType?: 'local' | 'session' | 'sync';
  url?: string;
  filePath?: string;
  requestId?: string;
}

export interface ChangeMetadata {
  sessionId: string;
  turnId: string;
  toolName: string;
  timestamp: number;
  userId?: string;
  rollbackable: boolean;
  description?: string;
  tags?: string[];
}

export interface DiffResult {
  changeId: string;
  type: string;
  operation: string;
  target: ChangeTarget;
  diff: ChangeDiff;
  metadata: ChangeMetadata;
  status: 'applied' | 'rolled_back' | 'failed';
  rollbackData?: any;
}

export interface ChangeDiff {
  before?: any;
  after?: any;
  delta?: any;
  size: number;
  checksum?: string;
}

export interface RollbackRequest {
  changeId?: string;
  changeIds?: string[];
  sessionId?: string;
  turnId?: string;
  until?: number;
}

export interface RollbackResult {
  success: boolean;
  rolledBackChanges: string[];
  failedChanges: RollbackFailure[];
  totalChanges: number;
}

export interface RollbackFailure {
  changeId: string;
  reason: string;
  error?: string;
}

export interface ChangeSnapshot {
  id: string;
  timestamp: number;
  changes: DiffResult[];
  metadata: {
    sessionId: string;
    turnId: string;
    description?: string;
  };
}

/**
 * DiffTracker implementation
 */
export class DiffTracker {
  private changes = new Map<string, DiffResult>();
  private snapshots = new Map<string, ChangeSnapshot>();
  private eventEmitter?: (event: Event) => void;
  private observingDOM = false;
  private domObserver?: MutationObserver;
  private storageHandlers = new Map<string, (event: StorageEvent) => void>();

  constructor(eventEmitter?: (event: Event) => void) {
    this.eventEmitter = eventEmitter;
  }

  /**
   * Add a change to be tracked
   */
  async addChange(request: AddChangeRequest): Promise<DiffResult> {
    // Generate diff information
    const diff = this.calculateDiff(request.before, request.after);

    // Create the diff result
    const result: DiffResult = {
      changeId: request.changeId,
      type: request.type,
      operation: request.operation,
      target: request.target,
      diff,
      metadata: request.metadata || this.createDefaultMetadata(request.changeId),
      status: 'applied',
      rollbackData: request.before,
    };

    // Store the change
    this.changes.set(request.changeId, result);

    // Emit change added event
    this.emitEvent({
      id: `evt_change_added_${request.changeId}`,
      msg: {
        type: 'ChangeAdded',
        data: {
          change_id: request.changeId,
          type: request.type,
          operation: request.operation,
          target: request.target,
        },
      },
    });

    // Start monitoring if this is a DOM change
    if (request.type === 'dom' && !this.observingDOM) {
      this.startDOMObservation();
    }

    // Start storage monitoring if this is a storage change
    if (request.type === 'storage') {
      this.startStorageObservation(request.target);
    }

    return result;
  }

  /**
   * Get changes based on filters
   */
  async getChanges(request: GetChangesRequest): Promise<DiffResult[]> {
    let filteredChanges = Array.from(this.changes.values());

    // Apply filters
    if (request.sessionId) {
      filteredChanges = filteredChanges.filter(c => c.metadata.sessionId === request.sessionId);
    }

    if (request.turnId) {
      filteredChanges = filteredChanges.filter(c => c.metadata.turnId === request.turnId);
    }

    if (request.type) {
      filteredChanges = filteredChanges.filter(c => c.type === request.type);
    }

    if (request.since) {
      filteredChanges = filteredChanges.filter(c => c.metadata.timestamp >= request.since!);
    }

    if (request.until) {
      filteredChanges = filteredChanges.filter(c => c.metadata.timestamp <= request.until!);
    }

    if (!request.includeRolledBack) {
      filteredChanges = filteredChanges.filter(c => c.status !== 'rolled_back');
    }

    // Sort by timestamp (most recent first)
    filteredChanges.sort((a, b) => b.metadata.timestamp - a.metadata.timestamp);

    if (request.limit) {
      filteredChanges = filteredChanges.slice(0, request.limit);
    }

    // Emit retrieval event
    this.emitEvent({
      id: `evt_changes_retrieved_${Date.now()}`,
      msg: {
        type: 'ChangesRetrieved',
        data: {
          filter: request,
          count: filteredChanges.length,
        },
      },
    });

    return filteredChanges;
  }

  /**
   * Rollback changes
   */
  async rollbackChanges(request: RollbackRequest): Promise<RollbackResult> {
    let targetChangeIds: string[] = [];

    // Determine which changes to rollback
    if (request.changeId) {
      targetChangeIds = [request.changeId];
      this.emitEvent({
        id: `evt_rollback_start_${request.changeId}`,
        msg: {
          type: 'RollbackStarted',
          data: {
            change_id: request.changeId,
            type: 'single_change',
          },
        },
      });
    } else if (request.changeIds) {
      targetChangeIds = request.changeIds;
      this.emitEvent({
        id: `evt_batch_rollback_${Date.now()}`,
        msg: {
          type: 'BatchRollbackStarted',
          data: {
            change_ids: request.changeIds,
            count: request.changeIds.length,
          },
        },
      });
    } else if (request.sessionId || request.turnId) {
      // Find all changes matching session/turn criteria
      const allChanges = Array.from(this.changes.values());
      targetChangeIds = allChanges
        .filter(c => {
          if (request.sessionId && c.metadata.sessionId !== request.sessionId) return false;
          if (request.turnId && c.metadata.turnId !== request.turnId) return false;
          if (request.until && c.metadata.timestamp > request.until) return false;
          return c.status === 'applied' && c.metadata.rollbackable;
        })
        .map(c => c.changeId);

      this.emitEvent({
        id: `evt_session_rollback_${Date.now()}`,
        msg: {
          type: 'SessionRollbackStarted',
          data: {
            session_id: request.sessionId,
            turn_id: request.turnId,
            until: request.until,
          },
        },
      });
    }

    const rolledBack: string[] = [];
    const failed: RollbackFailure[] = [];

    // Process each change
    for (const changeId of targetChangeIds) {
      const change = this.changes.get(changeId);
      if (!change) {
        failed.push({
          changeId,
          reason: 'Change not found',
          error: 'No change record with this ID',
        });
        continue;
      }

      if (change.status === 'rolled_back') {
        failed.push({
          changeId,
          reason: 'Already rolled back',
        });
        continue;
      }

      if (!change.metadata.rollbackable) {
        failed.push({
          changeId,
          reason: 'Change is not rollbackable',
        });
        continue;
      }

      try {
        await this.executeRollback(change);
        change.status = 'rolled_back';
        this.changes.set(changeId, change);
        rolledBack.push(changeId);
      } catch (error: any) {
        change.status = 'failed';
        this.changes.set(changeId, change);
        failed.push({
          changeId,
          reason: 'Rollback execution failed',
          error: error.message,
        });
      }
    }

    // Emit completion events
    if (request.changeId) {
      this.emitEvent({
        id: `evt_rollback_complete_${request.changeId}`,
        msg: {
          type: 'RollbackCompleted',
          data: {
            change_id: request.changeId,
            success: rolledBack.includes(request.changeId),
          },
        },
      });
    }

    return {
      success: rolledBack.length > 0,
      rolledBackChanges: rolledBack,
      failedChanges: failed,
      totalChanges: targetChangeIds.length,
    };
  }

  /**
   * Create a snapshot of current changes
   */
  async createSnapshot(sessionId: string, turnId: string, description?: string): Promise<ChangeSnapshot> {
    const snapshotId = `snapshot_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;

    // Get all changes for this session/turn
    const sessionChanges = Array.from(this.changes.values()).filter(c =>
      c.metadata.sessionId === sessionId &&
      c.metadata.turnId === turnId &&
      c.status === 'applied'
    );

    const snapshot: ChangeSnapshot = {
      id: snapshotId,
      timestamp: Date.now(),
      changes: [...sessionChanges], // Deep copy
      metadata: {
        sessionId,
        turnId,
        description,
      },
    };

    this.snapshots.set(snapshotId, snapshot);

    this.emitEvent({
      id: `evt_snapshot_created_${snapshotId}`,
      msg: {
        type: 'SnapshotCreated',
        data: {
          snapshot_id: snapshotId,
          session_id: sessionId,
          turn_id: turnId,
          change_count: sessionChanges.length,
        },
      },
    });

    return snapshot;
  }

  /**
   * Restore from a snapshot
   */
  async restoreSnapshot(snapshotId: string): Promise<RollbackResult> {
    const snapshot = this.snapshots.get(snapshotId);
    if (!snapshot) {
      return {
        success: false,
        rolledBackChanges: [],
        failedChanges: [{ changeId: snapshotId, reason: 'Snapshot not found' }],
        totalChanges: 0,
      };
    }

    // Rollback all changes that came after this snapshot
    const changeIds = snapshot.changes.map(c => c.changeId);

    this.emitEvent({
      id: `evt_snapshot_restored_${snapshotId}`,
      msg: {
        type: 'SnapshotRestored',
        data: {
          snapshot_id: snapshotId,
          change_count: snapshot.changes.length,
        },
      },
    });

    return this.rollbackChanges({ changeIds });
  }

  /**
   * Get snapshot by ID
   */
  async getSnapshot(snapshotId: string): Promise<ChangeSnapshot | null> {
    return this.snapshots.get(snapshotId) || null;
  }

  /**
   * Clear changes
   */
  async clearChanges(sessionId?: string, turnId?: string): Promise<number> {
    let clearedCount = 0;

    if (sessionId && turnId) {
      // Clear changes for specific session and turn
      const toDelete = Array.from(this.changes.entries()).filter(([, change]) =>
        change.metadata.sessionId === sessionId && change.metadata.turnId === turnId
      );

      toDelete.forEach(([changeId]) => {
        this.changes.delete(changeId);
        clearedCount++;
      });
    } else if (sessionId) {
      // Clear all changes for session
      const toDelete = Array.from(this.changes.entries()).filter(([, change]) =>
        change.metadata.sessionId === sessionId
      );

      toDelete.forEach(([changeId]) => {
        this.changes.delete(changeId);
        clearedCount++;
      });
    } else {
      // Clear all changes
      clearedCount = this.changes.size;
      this.changes.clear();
    }

    this.emitEvent({
      id: `evt_changes_cleared_${Date.now()}`,
      msg: {
        type: 'ChangesCleared',
        data: {
          session_id: sessionId,
          turn_id: turnId,
          cleared_count: clearedCount,
        },
      },
    });

    return clearedCount;
  }

  /**
   * Get all snapshots
   */
  getAllSnapshots(): ChangeSnapshot[] {
    return Array.from(this.snapshots.values())
      .sort((a, b) => b.timestamp - a.timestamp);
  }

  /**
   * Delete snapshot
   */
  deleteSnapshot(snapshotId: string): boolean {
    return this.snapshots.delete(snapshotId);
  }

  /**
   * Start DOM observation for automatic change detection
   */
  private startDOMObservation(): void {
    if (this.observingDOM || typeof document === 'undefined') {
      return;
    }

    this.domObserver = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        this.handleDOMMutation(mutation);
      });
    });

    this.domObserver.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeOldValue: true,
      characterData: true,
      characterDataOldValue: true,
    });

    this.observingDOM = true;
  }

  /**
   * Stop DOM observation
   */
  stopDOMObservation(): void {
    if (this.domObserver) {
      this.domObserver.disconnect();
      this.domObserver = undefined;
    }
    this.observingDOM = false;
  }

  /**
   * Handle DOM mutations
   */
  private handleDOMMutation(mutation: MutationRecord): void {
    // This would automatically track DOM changes
    // For now, we'll just log them as they are detected by our tools
    const changeId = `auto_dom_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;

    // This is a simplified version - in practice, you'd want more sophisticated tracking
    const changeData: AddChangeRequest = {
      changeId,
      type: 'dom',
      operation: mutation.type === 'childList' ? 'update' : 'update',
      target: {
        type: 'element',
        selector: this.getElementSelector(mutation.target as Element),
      },
      metadata: {
        sessionId: 'auto',
        turnId: 'dom_observation',
        toolName: 'dom_observer',
        timestamp: Date.now(),
        rollbackable: false, // Auto-detected changes are not rollbackable
        description: `Auto-detected ${mutation.type} mutation`,
      },
    };

    // Don't await this to avoid blocking the mutation observer
    this.addChange(changeData).catch(console.error);
  }

  /**
   * Start storage observation
   */
  private startStorageObservation(target: ChangeTarget): void {
    if (typeof window === 'undefined') return;

    const storageType = target.storageType || 'local';
    const handlerKey = `${storageType}_${target.storageKey}`;

    if (this.storageHandlers.has(handlerKey)) {
      return; // Already observing
    }

    const handler = (event: StorageEvent) => {
      if (event.key === target.storageKey && event.storageArea) {
        const changeId = `auto_storage_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;

        const changeData: AddChangeRequest = {
          changeId,
          type: 'storage',
          operation: event.newValue ? (event.oldValue ? 'update' : 'create') : 'delete',
          target,
          before: event.oldValue ? JSON.parse(event.oldValue) : undefined,
          after: event.newValue ? JSON.parse(event.newValue) : undefined,
          metadata: {
            sessionId: 'auto',
            turnId: 'storage_observation',
            toolName: 'storage_observer',
            timestamp: Date.now(),
            rollbackable: true, // Storage changes can be rolled back
            description: `Auto-detected storage ${event.newValue ? 'change' : 'removal'}`,
          },
        };

        this.addChange(changeData).catch(console.error);
      }
    };

    window.addEventListener('storage', handler);
    this.storageHandlers.set(handlerKey, handler);
  }

  /**
   * Stop storage observation
   */
  stopStorageObservation(target: ChangeTarget): void {
    if (typeof window === 'undefined') return;

    const storageType = target.storageType || 'local';
    const handlerKey = `${storageType}_${target.storageKey}`;
    const handler = this.storageHandlers.get(handlerKey);

    if (handler) {
      window.removeEventListener('storage', handler);
      this.storageHandlers.delete(handlerKey);
    }
  }

  /**
   * Execute rollback for a change
   */
  private async executeRollback(change: DiffResult): Promise<void> {
    switch (change.type) {
      case 'dom':
        await this.rollbackDOMChange(change);
        break;
      case 'storage':
        await this.rollbackStorageChange(change);
        break;
      case 'navigation':
        await this.rollbackNavigationChange(change);
        break;
      default:
        throw new Error(`Rollback not implemented for type: ${change.type}`);
    }
  }

  /**
   * Rollback DOM change
   */
  private async rollbackDOMChange(change: DiffResult): Promise<void> {
    const { target, rollbackData } = change;

    if (target.selector && typeof document !== 'undefined') {
      const element = document.querySelector(target.selector);
      if (!element) {
        throw new Error('DOM element no longer exists');
      }

      if (rollbackData) {
        // Restore previous state
        Object.entries(rollbackData).forEach(([key, value]) => {
          if (key === 'innerHTML') {
            element.innerHTML = value as string;
          } else if (key === 'textContent') {
            element.textContent = value as string;
          } else if (key.startsWith('data-')) {
            element.setAttribute(key, value as string);
          } else {
            (element as any)[key] = value;
          }
        });
      } else if (change.operation === 'create') {
        // Remove created element
        element.remove();
      }
    }
  }

  /**
   * Rollback storage change
   */
  private async rollbackStorageChange(change: DiffResult): Promise<void> {
    const { target, rollbackData } = change;

    if (typeof window !== 'undefined' && target.storageKey) {
      const storage = target.storageType === 'session' ? sessionStorage :
                    target.storageType === 'local' ? localStorage :
                    null;

      if (!storage) {
        throw new Error(`Unsupported storage type: ${target.storageType}`);
      }

      if (rollbackData !== undefined) {
        // Restore previous value
        storage.setItem(target.storageKey, JSON.stringify(rollbackData));
      } else {
        // Remove key that was created
        storage.removeItem(target.storageKey);
      }
    }
  }

  /**
   * Rollback navigation change
   */
  private async rollbackNavigationChange(change: DiffResult): Promise<void> {
    if (typeof window !== 'undefined' && change.rollbackData) {
      // Navigate back to previous URL
      window.history.pushState(null, '', change.rollbackData);
    }
  }

  /**
   * Calculate diff between before and after states
   */
  private calculateDiff(before: any, after: any): ChangeDiff {
    const delta = this.calculateDelta(before, after);
    const size = this.calculateSize(before, after);
    const checksum = this.generateChecksum(after);

    return {
      before,
      after,
      delta,
      size,
      checksum,
    };
  }

  /**
   * Calculate delta between states
   */
  private calculateDelta(before: any, after: any): any {
    if (!before) return { type: 'create', value: after };
    if (!after) return { type: 'delete', value: before };
    return { type: 'update', from: before, to: after };
  }

  /**
   * Calculate size difference
   */
  private calculateSize(before: any, after: any): number {
    const beforeSize = before ? JSON.stringify(before).length : 0;
    const afterSize = after ? JSON.stringify(after).length : 0;
    return Math.abs(afterSize - beforeSize);
  }

  /**
   * Generate checksum for data
   */
  private generateChecksum(data: any): string {
    if (!data) return '';

    // Simple checksum using btoa (base64 encoding)
    try {
      const jsonString = JSON.stringify(data);
      return btoa(jsonString).slice(0, 8);
    } catch {
      return '';
    }
  }

  /**
   * Get CSS selector for element
   */
  private getElementSelector(element: Element): string {
    if (element.id) {
      return `#${element.id}`;
    }

    if (element.className) {
      const classes = element.className.toString().split(' ').filter(c => c);
      if (classes.length > 0) {
        return `.${classes.join('.')}`;
      }
    }

    // Use tag name with nth-child if needed
    const tagName = element.tagName.toLowerCase();
    const parent = element.parentElement;

    if (parent) {
      const siblings = Array.from(parent.children).filter(child =>
        child.tagName.toLowerCase() === tagName
      );

      if (siblings.length > 1) {
        const index = siblings.indexOf(element) + 1;
        return `${tagName}:nth-child(${index})`;
      }
    }

    return tagName;
  }

  /**
   * Create default metadata
   */
  private createDefaultMetadata(changeId: string): ChangeMetadata {
    return {
      sessionId: 'unknown',
      turnId: 'unknown',
      toolName: 'diff_tracker',
      timestamp: Date.now(),
      rollbackable: true,
      description: `Change ${changeId}`,
    };
  }

  /**
   * Emit event if emitter is available
   */
  private emitEvent(event: Event): void {
    if (this.eventEmitter) {
      this.eventEmitter(event);
    }
  }

  /**
   * Cleanup - stop all observations
   */
  destroy(): void {
    this.stopDOMObservation();

    // Stop all storage observations
    if (typeof window !== 'undefined') {
      this.storageHandlers.forEach((handler) => {
        window.removeEventListener('storage', handler);
      });
    }
    this.storageHandlers.clear();

    // Clear all data
    this.changes.clear();
    this.snapshots.clear();
  }
}

// Export individual interfaces for easier imports
export type {
  AddChangeRequest as DiffAddChangeRequest,
  GetChangesRequest as DiffGetChangesRequest,
  RollbackRequest as DiffRollbackRequest,
};