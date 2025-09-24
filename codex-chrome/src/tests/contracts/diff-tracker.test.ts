/**
 * Contract tests for DiffTracker
 * Tests change tracking, diff generation, and rollback operations
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { EventCollector, createMockToolResult } from '../utils/test-helpers';

// Define DiffTracker contract interfaces
interface AddChangeRequest {
  changeId: string;
  type: 'dom' | 'storage' | 'navigation' | 'network' | 'file';
  operation: 'create' | 'update' | 'delete' | 'navigate';
  target: ChangeTarget;
  before?: any;
  after?: any;
  metadata?: ChangeMetadata;
}

interface GetChangesRequest {
  sessionId?: string;
  turnId?: string;
  type?: string;
  since?: number;
  until?: number;
  limit?: number;
  includeRolledBack?: boolean;
}

interface ChangeTarget {
  type: 'element' | 'storage_key' | 'url' | 'file_path' | 'network_request';
  selector?: string;
  storageKey?: string;
  storageType?: 'local' | 'session' | 'sync';
  url?: string;
  filePath?: string;
  requestId?: string;
}

interface ChangeMetadata {
  sessionId: string;
  turnId: string;
  toolName: string;
  timestamp: number;
  userId?: string;
  rollbackable: boolean;
  description?: string;
  tags?: string[];
}

interface DiffResult {
  changeId: string;
  type: string;
  operation: string;
  target: ChangeTarget;
  diff: ChangeDiff;
  metadata: ChangeMetadata;
  status: 'applied' | 'rolled_back' | 'failed';
  rollbackData?: any;
}

interface ChangeDiff {
  before?: any;
  after?: any;
  delta?: any;
  size: number;
  checksum?: string;
}

interface RollbackRequest {
  changeId?: string;
  changeIds?: string[];
  sessionId?: string;
  turnId?: string;
  until?: number;
}

interface RollbackResult {
  success: boolean;
  rolledBackChanges: string[];
  failedChanges: RollbackFailure[];
  totalChanges: number;
}

interface RollbackFailure {
  changeId: string;
  reason: string;
  error?: string;
}

interface ChangeSnapshot {
  id: string;
  timestamp: number;
  changes: DiffResult[];
  metadata: {
    sessionId: string;
    turnId: string;
    description?: string;
  };
}

interface DiffTracker {
  addChange(request: AddChangeRequest): Promise<DiffResult>;
  getChanges(request: GetChangesRequest): Promise<DiffResult[]>;
  rollbackChanges(request: RollbackRequest): Promise<RollbackResult>;
  createSnapshot(sessionId: string, turnId: string, description?: string): Promise<ChangeSnapshot>;
  restoreSnapshot(snapshotId: string): Promise<RollbackResult>;
  getSnapshot(snapshotId: string): Promise<ChangeSnapshot | null>;
  clearChanges(sessionId?: string, turnId?: string): Promise<number>;
}

describe('DiffTracker Contract', () => {
  let eventCollector: EventCollector;

  beforeEach(() => {
    eventCollector = new EventCollector();
  });

  describe('Change Tracking', () => {
    it('should handle AddChangeRequest and return DiffResult', async () => {
      const mockChanges = new Map<string, DiffResult>();

      const mockDiffTracker: DiffTracker = {
        async addChange(request: AddChangeRequest): Promise<DiffResult> {
          eventCollector.collect({
            id: 'evt_change_added',
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

          const diff: ChangeDiff = {
            before: request.before,
            after: request.after,
            delta: this.calculateDelta(request.before, request.after),
            size: this.calculateSize(request.before, request.after),
            checksum: this.generateChecksum(request.after),
          };

          const result: DiffResult = {
            changeId: request.changeId,
            type: request.type,
            operation: request.operation,
            target: request.target,
            diff,
            metadata: request.metadata!,
            status: 'applied',
            rollbackData: request.before,
          };

          mockChanges.set(request.changeId, result);
          return result;
        },

        async getChanges(): Promise<DiffResult[]> {
          return [];
        },
        async rollbackChanges(): Promise<RollbackResult> {
          return { success: true, rolledBackChanges: [], failedChanges: [], totalChanges: 0 };
        },
        async createSnapshot(): Promise<ChangeSnapshot> {
          throw new Error('Not implemented');
        },
        async restoreSnapshot(): Promise<RollbackResult> {
          throw new Error('Not implemented');
        },
        async getSnapshot(): Promise<ChangeSnapshot | null> {
          return null;
        },
        async clearChanges(): Promise<number> {
          return 0;
        },

        calculateDelta(before: any, after: any): any {
          if (!before) return { type: 'create', value: after };
          if (!after) return { type: 'delete', value: before };
          return { type: 'update', from: before, to: after };
        },

        calculateSize(before: any, after: any): number {
          const beforeSize = before ? JSON.stringify(before).length : 0;
          const afterSize = after ? JSON.stringify(after).length : 0;
          return Math.abs(afterSize - beforeSize);
        },

        generateChecksum(data: any): string {
          if (!data) return '';
          return btoa(JSON.stringify(data)).slice(0, 8);
        },
      };

      const request: AddChangeRequest = {
        changeId: 'change_1',
        type: 'dom',
        operation: 'update',
        target: {
          type: 'element',
          selector: '#submit-button',
        },
        before: { disabled: false, text: 'Submit' },
        after: { disabled: true, text: 'Submitting...' },
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_1',
          toolName: 'dom_tool',
          timestamp: Date.now(),
          rollbackable: true,
          description: 'Disable submit button during form submission',
        },
      };

      const result = await mockDiffTracker.addChange(request);

      // Verify result structure
      expect(result).toMatchObject({
        changeId: 'change_1',
        type: 'dom',
        operation: 'update',
        target: expect.objectContaining({
          type: 'element',
          selector: '#submit-button',
        }),
        diff: expect.objectContaining({
          before: { disabled: false, text: 'Submit' },
          after: { disabled: true, text: 'Submitting...' },
          delta: expect.objectContaining({
            type: 'update',
            from: { disabled: false, text: 'Submit' },
            to: { disabled: true, text: 'Submitting...' },
          }),
          size: expect.any(Number),
          checksum: expect.any(String),
        }),
        metadata: expect.objectContaining({
          sessionId: 'session_1',
          turnId: 'turn_1',
          toolName: 'dom_tool',
          rollbackable: true,
        }),
        status: 'applied',
        rollbackData: { disabled: false, text: 'Submit' },
      });

      // Verify event was emitted
      const changeEvent = eventCollector.findByType('ChangeAdded');
      expect(changeEvent).toBeDefined();
      expect((changeEvent?.msg as any).data.change_id).toBe('change_1');
    });

    it('should handle different change types', async () => {
      const mockDiffTracker: DiffTracker = {
        async addChange(request: AddChangeRequest): Promise<DiffResult> {
          const result: DiffResult = {
            changeId: request.changeId,
            type: request.type,
            operation: request.operation,
            target: request.target,
            diff: {
              before: request.before,
              after: request.after,
              size: 0,
            },
            metadata: request.metadata!,
            status: 'applied',
          };

          eventCollector.collect({
            id: `evt_${request.type}_change`,
            msg: {
              type: 'ChangeAdded',
              data: {
                change_id: request.changeId,
                type: request.type,
              },
            },
          });

          return result;
        },
        async getChanges(): Promise<DiffResult[]> {
          return [];
        },
        async rollbackChanges(): Promise<RollbackResult> {
          return { success: true, rolledBackChanges: [], failedChanges: [], totalChanges: 0 };
        },
        async createSnapshot(): Promise<ChangeSnapshot> {
          throw new Error('Not implemented');
        },
        async restoreSnapshot(): Promise<RollbackResult> {
          throw new Error('Not implemented');
        },
        async getSnapshot(): Promise<ChangeSnapshot | null> {
          return null;
        },
        async clearChanges(): Promise<number> {
          return 0;
        },
      };

      // Test DOM change
      const domChange: AddChangeRequest = {
        changeId: 'dom_1',
        type: 'dom',
        operation: 'create',
        target: { type: 'element', selector: 'div.new-element' },
        after: { innerHTML: 'New content' },
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_1',
          toolName: 'dom_tool',
          timestamp: Date.now(),
          rollbackable: true,
        },
      };

      // Test storage change
      const storageChange: AddChangeRequest = {
        changeId: 'storage_1',
        type: 'storage',
        operation: 'update',
        target: { type: 'storage_key', storageKey: 'user_preference', storageType: 'local' },
        before: { theme: 'light' },
        after: { theme: 'dark' },
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_1',
          toolName: 'storage_tool',
          timestamp: Date.now(),
          rollbackable: true,
        },
      };

      // Test navigation change
      const navigationChange: AddChangeRequest = {
        changeId: 'nav_1',
        type: 'navigation',
        operation: 'navigate',
        target: { type: 'url', url: 'https://example.com/page2' },
        before: 'https://example.com/page1',
        after: 'https://example.com/page2',
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_1',
          toolName: 'navigation_tool',
          timestamp: Date.now(),
          rollbackable: true,
        },
      };

      const domResult = await mockDiffTracker.addChange(domChange);
      const storageResult = await mockDiffTracker.addChange(storageChange);
      const navResult = await mockDiffTracker.addChange(navigationChange);

      expect(domResult.type).toBe('dom');
      expect(storageResult.type).toBe('storage');
      expect(navResult.type).toBe('navigation');

      const events = eventCollector.getEvents();
      expect(events).toHaveLength(3);
    });
  });

  describe('Change Retrieval', () => {
    it('should handle GetChangesRequest with various filters', async () => {
      const mockChanges: DiffResult[] = [
        {
          changeId: 'change_1',
          type: 'dom',
          operation: 'update',
          target: { type: 'element', selector: '#button1' },
          diff: { size: 10 },
          metadata: {
            sessionId: 'session_1',
            turnId: 'turn_1',
            toolName: 'dom_tool',
            timestamp: Date.now() - 3000,
            rollbackable: true,
          },
          status: 'applied',
        },
        {
          changeId: 'change_2',
          type: 'storage',
          operation: 'create',
          target: { type: 'storage_key', storageKey: 'newKey' },
          diff: { size: 20 },
          metadata: {
            sessionId: 'session_1',
            turnId: 'turn_2',
            toolName: 'storage_tool',
            timestamp: Date.now() - 2000,
            rollbackable: true,
          },
          status: 'applied',
        },
        {
          changeId: 'change_3',
          type: 'dom',
          operation: 'delete',
          target: { type: 'element', selector: '#removed' },
          diff: { size: 5 },
          metadata: {
            sessionId: 'session_2',
            turnId: 'turn_1',
            toolName: 'dom_tool',
            timestamp: Date.now() - 1000,
            rollbackable: false,
          },
          status: 'rolled_back',
        },
      ];

      const mockDiffTracker: DiffTracker = {
        async addChange(): Promise<DiffResult> {
          throw new Error('Not implemented');
        },
        async getChanges(request: GetChangesRequest): Promise<DiffResult[]> {
          let filteredChanges = [...mockChanges];

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

          if (request.limit) {
            filteredChanges = filteredChanges.slice(0, request.limit);
          }

          eventCollector.collect({
            id: 'evt_changes_retrieved',
            msg: {
              type: 'ChangesRetrieved',
              data: {
                filter: request,
                count: filteredChanges.length,
              },
            },
          });

          return filteredChanges;
        },
        async rollbackChanges(): Promise<RollbackResult> {
          return { success: true, rolledBackChanges: [], failedChanges: [], totalChanges: 0 };
        },
        async createSnapshot(): Promise<ChangeSnapshot> {
          throw new Error('Not implemented');
        },
        async restoreSnapshot(): Promise<RollbackResult> {
          throw new Error('Not implemented');
        },
        async getSnapshot(): Promise<ChangeSnapshot | null> {
          return null;
        },
        async clearChanges(): Promise<number> {
          return 0;
        },
      };

      // Test filtering by session
      const sessionRequest: GetChangesRequest = { sessionId: 'session_1' };
      const sessionResults = await mockDiffTracker.getChanges(sessionRequest);
      expect(sessionResults).toHaveLength(2);
      expect(sessionResults.every(c => c.metadata.sessionId === 'session_1')).toBe(true);

      // Test filtering by type
      const typeRequest: GetChangesRequest = { type: 'dom' };
      const typeResults = await mockDiffTracker.getChanges(typeRequest);
      expect(typeResults).toHaveLength(2);
      expect(typeResults.every(c => c.type === 'dom')).toBe(true);

      // Test excluding rolled back changes
      const activeRequest: GetChangesRequest = { includeRolledBack: false };
      const activeResults = await mockDiffTracker.getChanges(activeRequest);
      expect(activeResults).toHaveLength(2);
      expect(activeResults.every(c => c.status !== 'rolled_back')).toBe(true);

      // Test with limit
      const limitRequest: GetChangesRequest = { limit: 1 };
      const limitResults = await mockDiffTracker.getChanges(limitRequest);
      expect(limitResults).toHaveLength(1);

      const retrievalEvents = eventCollector.filterByType('ChangesRetrieved');
      expect(retrievalEvents).toHaveLength(4);
    });
  });

  describe('Rollback Operations', () => {
    it('should handle single change rollback', async () => {
      const mockDiffTracker: DiffTracker = {
        async addChange(): Promise<DiffResult> {
          throw new Error('Not implemented');
        },
        async getChanges(): Promise<DiffResult[]> {
          return [];
        },
        async rollbackChanges(request: RollbackRequest): Promise<RollbackResult> {
          if (!request.changeId) {
            return {
              success: false,
              rolledBackChanges: [],
              failedChanges: [{ changeId: 'unknown', reason: 'No change ID provided' }],
              totalChanges: 0,
            };
          }

          eventCollector.collect({
            id: 'evt_rollback_start',
            msg: {
              type: 'RollbackStarted',
              data: {
                change_id: request.changeId,
                type: 'single_change',
              },
            },
          });

          // Simulate rollback process
          await new Promise(resolve => setTimeout(resolve, 10));

          eventCollector.collect({
            id: 'evt_rollback_complete',
            msg: {
              type: 'RollbackCompleted',
              data: {
                change_id: request.changeId,
                success: true,
              },
            },
          });

          return {
            success: true,
            rolledBackChanges: [request.changeId],
            failedChanges: [],
            totalChanges: 1,
          };
        },
        async createSnapshot(): Promise<ChangeSnapshot> {
          throw new Error('Not implemented');
        },
        async restoreSnapshot(): Promise<RollbackResult> {
          throw new Error('Not implemented');
        },
        async getSnapshot(): Promise<ChangeSnapshot | null> {
          return null;
        },
        async clearChanges(): Promise<number> {
          return 0;
        },
      };

      const rollbackRequest: RollbackRequest = { changeId: 'change_1' };
      const result = await mockDiffTracker.rollbackChanges(rollbackRequest);

      expect(result.success).toBe(true);
      expect(result.rolledBackChanges).toEqual(['change_1']);
      expect(result.failedChanges).toHaveLength(0);
      expect(result.totalChanges).toBe(1);

      const startEvent = eventCollector.findByType('RollbackStarted');
      const completeEvent = eventCollector.findByType('RollbackCompleted');
      expect(startEvent).toBeDefined();
      expect(completeEvent).toBeDefined();
    });

    it('should handle batch rollback', async () => {
      const mockDiffTracker: DiffTracker = {
        async addChange(): Promise<DiffResult> {
          throw new Error('Not implemented');
        },
        async getChanges(): Promise<DiffResult[]> {
          return [];
        },
        async rollbackChanges(request: RollbackRequest): Promise<RollbackResult> {
          const changeIds = request.changeIds || [];

          eventCollector.collect({
            id: 'evt_batch_rollback',
            msg: {
              type: 'BatchRollbackStarted',
              data: {
                change_ids: changeIds,
                count: changeIds.length,
              },
            },
          });

          // Simulate some changes failing to rollback
          const rolledBack = changeIds.slice(0, -1); // All but last
          const failed = changeIds.slice(-1); // Last one fails

          const failedChanges: RollbackFailure[] = failed.map(id => ({
            changeId: id,
            reason: 'Change is not rollbackable',
            error: 'DOM element no longer exists',
          }));

          return {
            success: rolledBack.length > 0,
            rolledBackChanges: rolledBack,
            failedChanges,
            totalChanges: changeIds.length,
          };
        },
        async createSnapshot(): Promise<ChangeSnapshot> {
          throw new Error('Not implemented');
        },
        async restoreSnapshot(): Promise<RollbackResult> {
          throw new Error('Not implemented');
        },
        async getSnapshot(): Promise<ChangeSnapshot | null> {
          return null;
        },
        async clearChanges(): Promise<number> {
          return 0;
        },
      };

      const batchRequest: RollbackRequest = {
        changeIds: ['change_1', 'change_2', 'change_3'],
      };

      const result = await mockDiffTracker.rollbackChanges(batchRequest);

      expect(result.success).toBe(true);
      expect(result.rolledBackChanges).toEqual(['change_1', 'change_2']);
      expect(result.failedChanges).toHaveLength(1);
      expect(result.failedChanges[0]).toMatchObject({
        changeId: 'change_3',
        reason: 'Change is not rollbackable',
        error: 'DOM element no longer exists',
      });
      expect(result.totalChanges).toBe(3);

      const batchEvent = eventCollector.findByType('BatchRollbackStarted');
      expect(batchEvent).toBeDefined();
    });

    it('should handle rollback by session/turn', async () => {
      const mockDiffTracker: DiffTracker = {
        async addChange(): Promise<DiffResult> {
          throw new Error('Not implemented');
        },
        async getChanges(): Promise<DiffResult[]> {
          return [];
        },
        async rollbackChanges(request: RollbackRequest): Promise<RollbackResult> {
          eventCollector.collect({
            id: 'evt_session_rollback',
            msg: {
              type: 'SessionRollbackStarted',
              data: {
                session_id: request.sessionId,
                turn_id: request.turnId,
                until: request.until,
              },
            },
          });

          // Simulate rolling back all changes in session/turn
          const mockRolledBack = ['change_1', 'change_2', 'change_3'];

          return {
            success: true,
            rolledBackChanges: mockRolledBack,
            failedChanges: [],
            totalChanges: mockRolledBack.length,
          };
        },
        async createSnapshot(): Promise<ChangeSnapshot> {
          throw new Error('Not implemented');
        },
        async restoreSnapshot(): Promise<RollbackResult> {
          throw new Error('Not implemented');
        },
        async getSnapshot(): Promise<ChangeSnapshot | null> {
          return null;
        },
        async clearChanges(): Promise<number> {
          return 0;
        },
      };

      const sessionRequest: RollbackRequest = {
        sessionId: 'session_1',
        turnId: 'turn_2',
      };

      const result = await mockDiffTracker.rollbackChanges(sessionRequest);

      expect(result.success).toBe(true);
      expect(result.rolledBackChanges).toHaveLength(3);
      expect(result.failedChanges).toHaveLength(0);

      const sessionEvent = eventCollector.findByType('SessionRollbackStarted');
      expect(sessionEvent).toBeDefined();
    });
  });

  describe('Snapshots', () => {
    it('should create and restore snapshots', async () => {
      const mockSnapshots = new Map<string, ChangeSnapshot>();

      const mockDiffTracker: DiffTracker = {
        async addChange(): Promise<DiffResult> {
          throw new Error('Not implemented');
        },
        async getChanges(): Promise<DiffResult[]> {
          return [];
        },
        async rollbackChanges(): Promise<RollbackResult> {
          return { success: true, rolledBackChanges: [], failedChanges: [], totalChanges: 0 };
        },
        async createSnapshot(sessionId: string, turnId: string, description?: string): Promise<ChangeSnapshot> {
          const snapshotId = `snapshot_${Date.now()}`;
          const mockChanges: DiffResult[] = [
            {
              changeId: 'change_1',
              type: 'dom',
              operation: 'update',
              target: { type: 'element', selector: '#test' },
              diff: { size: 10 },
              metadata: {
                sessionId,
                turnId,
                toolName: 'dom_tool',
                timestamp: Date.now(),
                rollbackable: true,
              },
              status: 'applied',
            },
          ];

          const snapshot: ChangeSnapshot = {
            id: snapshotId,
            timestamp: Date.now(),
            changes: mockChanges,
            metadata: {
              sessionId,
              turnId,
              description,
            },
          };

          mockSnapshots.set(snapshotId, snapshot);

          eventCollector.collect({
            id: 'evt_snapshot_created',
            msg: {
              type: 'SnapshotCreated',
              data: {
                snapshot_id: snapshotId,
                session_id: sessionId,
                turn_id: turnId,
                change_count: mockChanges.length,
              },
            },
          });

          return snapshot;
        },
        async restoreSnapshot(snapshotId: string): Promise<RollbackResult> {
          const snapshot = mockSnapshots.get(snapshotId);
          if (!snapshot) {
            return {
              success: false,
              rolledBackChanges: [],
              failedChanges: [{ changeId: snapshotId, reason: 'Snapshot not found' }],
              totalChanges: 0,
            };
          }

          eventCollector.collect({
            id: 'evt_snapshot_restored',
            msg: {
              type: 'SnapshotRestored',
              data: {
                snapshot_id: snapshotId,
                change_count: snapshot.changes.length,
              },
            },
          });

          const changeIds = snapshot.changes.map(c => c.changeId);
          return {
            success: true,
            rolledBackChanges: changeIds,
            failedChanges: [],
            totalChanges: changeIds.length,
          };
        },
        async getSnapshot(snapshotId: string): Promise<ChangeSnapshot | null> {
          return mockSnapshots.get(snapshotId) || null;
        },
        async clearChanges(): Promise<number> {
          return 0;
        },
      };

      // Create snapshot
      const snapshot = await mockDiffTracker.createSnapshot(
        'session_1',
        'turn_1',
        'Before risky operation'
      );

      expect(snapshot).toMatchObject({
        id: expect.any(String),
        timestamp: expect.any(Number),
        changes: expect.arrayContaining([
          expect.objectContaining({
            changeId: 'change_1',
            type: 'dom',
          }),
        ]),
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_1',
          description: 'Before risky operation',
        },
      });

      const createEvent = eventCollector.findByType('SnapshotCreated');
      expect(createEvent).toBeDefined();

      // Restore snapshot
      const restoreResult = await mockDiffTracker.restoreSnapshot(snapshot.id);

      expect(restoreResult.success).toBe(true);
      expect(restoreResult.rolledBackChanges).toEqual(['change_1']);

      const restoreEvent = eventCollector.findByType('SnapshotRestored');
      expect(restoreEvent).toBeDefined();

      // Get snapshot
      const retrievedSnapshot = await mockDiffTracker.getSnapshot(snapshot.id);
      expect(retrievedSnapshot).toEqual(snapshot);
    });
  });

  describe('Change Management', () => {
    it('should support change clearing', async () => {
      const mockDiffTracker: DiffTracker = {
        async addChange(): Promise<DiffResult> {
          throw new Error('Not implemented');
        },
        async getChanges(): Promise<DiffResult[]> {
          return [];
        },
        async rollbackChanges(): Promise<RollbackResult> {
          return { success: true, rolledBackChanges: [], failedChanges: [], totalChanges: 0 };
        },
        async createSnapshot(): Promise<ChangeSnapshot> {
          throw new Error('Not implemented');
        },
        async restoreSnapshot(): Promise<RollbackResult> {
          throw new Error('Not implemented');
        },
        async getSnapshot(): Promise<ChangeSnapshot | null> {
          return null;
        },
        async clearChanges(sessionId?: string, turnId?: string): Promise<number> {
          let clearedCount = 0;

          if (sessionId && turnId) {
            clearedCount = 3; // Mock: 3 changes cleared for specific session/turn
          } else if (sessionId) {
            clearedCount = 7; // Mock: 7 changes cleared for session
          } else {
            clearedCount = 15; // Mock: 15 total changes cleared
          }

          eventCollector.collect({
            id: 'evt_changes_cleared',
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
        },
      };

      // Clear all changes
      const allCleared = await mockDiffTracker.clearChanges();
      expect(allCleared).toBe(15);

      // Clear by session
      const sessionCleared = await mockDiffTracker.clearChanges('session_1');
      expect(sessionCleared).toBe(7);

      // Clear by session and turn
      const turnCleared = await mockDiffTracker.clearChanges('session_1', 'turn_1');
      expect(turnCleared).toBe(3);

      const clearEvents = eventCollector.filterByType('ChangesCleared');
      expect(clearEvents).toHaveLength(3);
    });
  });
});