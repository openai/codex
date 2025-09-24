/**
 * Contract tests for ApprovalManager
 * Tests approval request/response handling, policies, and timeout scenarios
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { EventCollector, createDeferred, waitFor } from '../utils/test-helpers';
import { ReviewDecision } from '../../protocol/types';

// Define ApprovalManager contract interfaces
interface ApprovalRequest {
  id: string;
  type: 'command' | 'file_operation' | 'network_access' | 'storage_access' | 'dangerous_action';
  title: string;
  description: string;
  details: ApprovalDetails;
  metadata?: ApprovalMetadata;
  timeout?: number;
  policy?: ApprovalPolicy;
}

interface ApprovalDetails {
  command?: string;
  filePath?: string;
  url?: string;
  action?: string;
  parameters?: Record<string, any>;
  riskLevel: 'low' | 'medium' | 'high' | 'critical';
  impact?: string[];
}

interface ApprovalMetadata {
  sessionId: string;
  turnId: string;
  toolName: string;
  timestamp: number;
  source?: string;
}

interface ApprovalResponse {
  id: string;
  decision: ReviewDecision;
  timestamp: number;
  reason?: string;
  modifications?: Record<string, any>;
  metadata?: Record<string, any>;
}

interface ApprovalPolicy {
  mode: 'always_ask' | 'auto_approve_safe' | 'auto_reject_unsafe' | 'never_ask';
  riskThreshold?: 'low' | 'medium' | 'high';
  trustedDomains?: string[];
  allowedCommands?: string[];
  blockedCommands?: string[];
  timeout?: number;
}

interface ApprovalStatus {
  id: string;
  status: 'pending' | 'approved' | 'rejected' | 'timeout' | 'canceled';
  decision?: ReviewDecision;
  timestamp: number;
  timeRemaining?: number;
  policy?: ApprovalPolicy;
}

interface ApprovalManager {
  requestApproval(request: ApprovalRequest): Promise<ApprovalResponse>;
  handleDecision(response: ApprovalResponse): Promise<void>;
  getStatus(id: string): ApprovalStatus | null;
  cancelRequest(id: string): Promise<boolean>;
  updatePolicy(policy: Partial<ApprovalPolicy>): Promise<void>;
  getPolicy(): ApprovalPolicy;
}

describe('ApprovalManager Contract', () => {
  let eventCollector: EventCollector;

  beforeEach(() => {
    eventCollector = new EventCollector();
  });

  describe('Approval Request/Response', () => {
    it('should handle ApprovalRequest and return ApprovalResponse', async () => {
      const mockApprovalManager: ApprovalManager = {
        async requestApproval(request: ApprovalRequest): Promise<ApprovalResponse> {
          eventCollector.collect({
            id: 'evt_approval_requested',
            msg: {
              type: 'ApprovalRequested',
              data: {
                approval_id: request.id,
                type: request.type,
                risk_level: request.details.riskLevel,
                title: request.title,
              },
            },
          });

          // Simulate user approval after delay
          await new Promise(resolve => setTimeout(resolve, 10));

          eventCollector.collect({
            id: 'evt_approval_granted',
            msg: {
              type: 'ApprovalGranted',
              data: {
                approval_id: request.id,
                decision: 'approve',
              },
            },
          });

          return {
            id: request.id,
            decision: 'approve',
            timestamp: Date.now(),
            reason: 'User approved action',
          };
        },
        async handleDecision(): Promise<void> {},
        getStatus(): ApprovalStatus | null {
          return null;
        },
        async cancelRequest(): Promise<boolean> {
          return false;
        },
        async updatePolicy(): Promise<void> {},
        getPolicy(): ApprovalPolicy {
          return { mode: 'always_ask' };
        },
      };

      const request: ApprovalRequest = {
        id: 'approval_1',
        type: 'command',
        title: 'Execute Shell Command',
        description: 'Run a shell command to create a directory',
        details: {
          command: 'mkdir /tmp/test-dir',
          riskLevel: 'low',
          impact: ['file_system_write'],
        },
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_1',
          toolName: 'shell_exec',
          timestamp: Date.now(),
        },
        timeout: 30000,
      };

      const response = await mockApprovalManager.requestApproval(request);

      // Verify response structure
      expect(response).toMatchObject({
        id: 'approval_1',
        decision: 'approve',
        timestamp: expect.any(Number),
        reason: expect.any(String),
      });

      // Verify events were emitted
      const events = eventCollector.getEvents();
      expect(events).toHaveLength(2);
      expect(events[0].msg.type).toBe('ApprovalRequested');
      expect(events[1].msg.type).toBe('ApprovalGranted');
    });

    it('should handle approval rejection', async () => {
      const mockApprovalManager: ApprovalManager = {
        async requestApproval(request: ApprovalRequest): Promise<ApprovalResponse> {
          eventCollector.collect({
            id: 'evt_approval_requested',
            msg: {
              type: 'ApprovalRequested',
              data: {
                approval_id: request.id,
                type: request.type,
                risk_level: request.details.riskLevel,
              },
            },
          });

          // Simulate user rejection
          eventCollector.collect({
            id: 'evt_approval_rejected',
            msg: {
              type: 'ApprovalRejected',
              data: {
                approval_id: request.id,
                decision: 'reject',
                reason: 'High risk action rejected by user',
              },
            },
          });

          return {
            id: request.id,
            decision: 'reject',
            timestamp: Date.now(),
            reason: 'High risk action rejected by user',
          };
        },
        async handleDecision(): Promise<void> {},
        getStatus(): ApprovalStatus | null {
          return null;
        },
        async cancelRequest(): Promise<boolean> {
          return false;
        },
        async updatePolicy(): Promise<void> {},
        getPolicy(): ApprovalPolicy {
          return { mode: 'always_ask' };
        },
      };

      const request: ApprovalRequest = {
        id: 'approval_reject',
        type: 'dangerous_action',
        title: 'Delete System Files',
        description: 'This action will permanently delete system files',
        details: {
          command: 'rm -rf /system/*',
          riskLevel: 'critical',
          impact: ['data_loss', 'system_damage'],
        },
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_2',
          toolName: 'shell_exec',
          timestamp: Date.now(),
        },
      };

      const response = await mockApprovalManager.requestApproval(request);

      expect(response.decision).toBe('reject');
      expect(response.reason).toContain('rejected');

      const rejectionEvent = eventCollector.findByType('ApprovalRejected');
      expect(rejectionEvent).toBeDefined();
    });

    it('should handle request_change decision', async () => {
      const mockApprovalManager: ApprovalManager = {
        async requestApproval(request: ApprovalRequest): Promise<ApprovalResponse> {
          eventCollector.collect({
            id: 'evt_approval_requested',
            msg: {
              type: 'ApprovalRequested',
              data: {
                approval_id: request.id,
                type: request.type,
              },
            },
          });

          eventCollector.collect({
            id: 'evt_approval_modified',
            msg: {
              type: 'ApprovalModified',
              data: {
                approval_id: request.id,
                decision: 'request_change',
                modifications: { targetPath: '/tmp/safe-dir' },
              },
            },
          });

          return {
            id: request.id,
            decision: 'request_change',
            timestamp: Date.now(),
            reason: 'User requested safer path',
            modifications: {
              targetPath: '/tmp/safe-dir',
            },
          };
        },
        async handleDecision(): Promise<void> {},
        getStatus(): ApprovalStatus | null {
          return null;
        },
        async cancelRequest(): Promise<boolean> {
          return false;
        },
        async updatePolicy(): Promise<void> {},
        getPolicy(): ApprovalPolicy {
          return { mode: 'always_ask' };
        },
      };

      const request: ApprovalRequest = {
        id: 'approval_modify',
        type: 'file_operation',
        title: 'Create Directory',
        description: 'Create a directory in system location',
        details: {
          filePath: '/system/new-dir',
          action: 'create_directory',
          riskLevel: 'medium',
        },
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_3',
          toolName: 'file_manager',
          timestamp: Date.now(),
        },
      };

      const response = await mockApprovalManager.requestApproval(request);

      expect(response.decision).toBe('request_change');
      expect(response.modifications).toBeDefined();
      expect(response.modifications?.targetPath).toBe('/tmp/safe-dir');

      const modificationEvent = eventCollector.findByType('ApprovalModified');
      expect(modificationEvent).toBeDefined();
    });
  });

  describe('Approval Policies', () => {
    it('should support auto-approval for low-risk actions', async () => {
      const autoApprovePolicy: ApprovalPolicy = {
        mode: 'auto_approve_safe',
        riskThreshold: 'low',
        trustedDomains: ['localhost', '*.example.com'],
        allowedCommands: ['ls', 'pwd', 'echo'],
      };

      const mockApprovalManager: ApprovalManager = {
        async requestApproval(request: ApprovalRequest): Promise<ApprovalResponse> {
          const policy = this.getPolicy();

          if (
            policy.mode === 'auto_approve_safe' &&
            request.details.riskLevel === 'low' &&
            (request.details.command ? policy.allowedCommands?.includes(request.details.command.split(' ')[0]) : true)
          ) {
            eventCollector.collect({
              id: 'evt_auto_approved',
              msg: {
                type: 'AutoApproved',
                data: {
                  approval_id: request.id,
                  policy_reason: 'Low risk action auto-approved',
                },
              },
            });

            return {
              id: request.id,
              decision: 'approve',
              timestamp: Date.now(),
              reason: 'Auto-approved by policy',
              metadata: { autoApproved: true },
            };
          }

          return {
            id: request.id,
            decision: 'reject',
            timestamp: Date.now(),
            reason: 'Not eligible for auto-approval',
          };
        },
        async handleDecision(): Promise<void> {},
        getStatus(): ApprovalStatus | null {
          return null;
        },
        async cancelRequest(): Promise<boolean> {
          return false;
        },
        async updatePolicy(): Promise<void> {},
        getPolicy(): ApprovalPolicy {
          return autoApprovePolicy;
        },
      };

      const lowRiskRequest: ApprovalRequest = {
        id: 'approval_auto',
        type: 'command',
        title: 'List Directory',
        description: 'List files in current directory',
        details: {
          command: 'ls -la',
          riskLevel: 'low',
          impact: ['read_access'],
        },
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_4',
          toolName: 'shell_exec',
          timestamp: Date.now(),
        },
      };

      const response = await mockApprovalManager.requestApproval(lowRiskRequest);

      expect(response.decision).toBe('approve');
      expect(response.metadata?.autoApproved).toBe(true);

      const autoApproveEvent = eventCollector.findByType('AutoApproved');
      expect(autoApproveEvent).toBeDefined();
    });

    it('should support auto-rejection for high-risk actions', async () => {
      const autoRejectPolicy: ApprovalPolicy = {
        mode: 'auto_reject_unsafe',
        riskThreshold: 'medium',
        blockedCommands: ['rm', 'del', 'format', 'fdisk'],
      };

      const mockApprovalManager: ApprovalManager = {
        async requestApproval(request: ApprovalRequest): Promise<ApprovalResponse> {
          const policy = this.getPolicy();

          if (
            policy.mode === 'auto_reject_unsafe' &&
            (
              request.details.riskLevel === 'high' ||
              request.details.riskLevel === 'critical' ||
              (request.details.command && policy.blockedCommands?.some(cmd =>
                request.details.command!.includes(cmd)
              ))
            )
          ) {
            eventCollector.collect({
              id: 'evt_auto_rejected',
              msg: {
                type: 'AutoRejected',
                data: {
                  approval_id: request.id,
                  policy_reason: 'High risk action auto-rejected',
                },
              },
            });

            return {
              id: request.id,
              decision: 'reject',
              timestamp: Date.now(),
              reason: 'Auto-rejected by policy',
              metadata: { autoRejected: true },
            };
          }

          return {
            id: request.id,
            decision: 'approve',
            timestamp: Date.now(),
            reason: 'Allowed by policy',
          };
        },
        async handleDecision(): Promise<void> {},
        getStatus(): ApprovalStatus | null {
          return null;
        },
        async cancelRequest(): Promise<boolean> {
          return false;
        },
        async updatePolicy(): Promise<void> {},
        getPolicy(): ApprovalPolicy {
          return autoRejectPolicy;
        },
      };

      const highRiskRequest: ApprovalRequest = {
        id: 'approval_danger',
        type: 'dangerous_action',
        title: 'Delete Files',
        description: 'Delete all files in directory',
        details: {
          command: 'rm -rf /important/data/*',
          riskLevel: 'critical',
          impact: ['data_loss', 'irreversible'],
        },
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_5',
          toolName: 'shell_exec',
          timestamp: Date.now(),
        },
      };

      const response = await mockApprovalManager.requestApproval(highRiskRequest);

      expect(response.decision).toBe('reject');
      expect(response.metadata?.autoRejected).toBe(true);

      const autoRejectEvent = eventCollector.findByType('AutoRejected');
      expect(autoRejectEvent).toBeDefined();
    });

    it('should support policy updates', async () => {
      let currentPolicy: ApprovalPolicy = {
        mode: 'always_ask',
        riskThreshold: 'medium',
      };

      const mockApprovalManager: ApprovalManager = {
        async requestApproval(): Promise<ApprovalResponse> {
          return {
            id: 'test',
            decision: 'approve',
            timestamp: Date.now(),
          };
        },
        async handleDecision(): Promise<void> {},
        getStatus(): ApprovalStatus | null {
          return null;
        },
        async cancelRequest(): Promise<boolean> {
          return false;
        },
        async updatePolicy(updates: Partial<ApprovalPolicy>): Promise<void> {
          currentPolicy = { ...currentPolicy, ...updates };

          eventCollector.collect({
            id: 'evt_policy_updated',
            msg: {
              type: 'PolicyUpdated',
              data: {
                policy: currentPolicy,
                changes: updates,
              },
            },
          });
        },
        getPolicy(): ApprovalPolicy {
          return currentPolicy;
        },
      };

      const policyUpdates = {
        mode: 'auto_approve_safe' as const,
        riskThreshold: 'low' as const,
        trustedDomains: ['localhost', 'example.com'],
      };

      await mockApprovalManager.updatePolicy(policyUpdates);

      const updatedPolicy = mockApprovalManager.getPolicy();
      expect(updatedPolicy.mode).toBe('auto_approve_safe');
      expect(updatedPolicy.riskThreshold).toBe('low');
      expect(updatedPolicy.trustedDomains).toEqual(['localhost', 'example.com']);

      const policyEvent = eventCollector.findByType('PolicyUpdated');
      expect(policyEvent).toBeDefined();
    });
  });

  describe('Timeout Scenarios', () => {
    it('should handle approval timeout', async () => {
      const deferred = createDeferred<ApprovalResponse>();

      const mockApprovalManager: ApprovalManager = {
        async requestApproval(request: ApprovalRequest): Promise<ApprovalResponse> {
          eventCollector.collect({
            id: 'evt_approval_requested',
            msg: {
              type: 'ApprovalRequested',
              data: {
                approval_id: request.id,
                timeout_ms: request.timeout,
              },
            },
          });

          // Set up timeout
          const timeout = request.timeout || 30000;
          const timeoutPromise = new Promise<ApprovalResponse>((resolve) => {
            setTimeout(() => {
              eventCollector.collect({
                id: 'evt_approval_timeout',
                msg: {
                  type: 'ApprovalTimeout',
                  data: {
                    approval_id: request.id,
                    timeout_ms: timeout,
                  },
                },
              });

              resolve({
                id: request.id,
                decision: 'reject',
                timestamp: Date.now(),
                reason: 'Request timed out',
                metadata: { timeout: true },
              });
            }, timeout);
          });

          // Race between user decision and timeout
          return Promise.race([deferred.promise, timeoutPromise]);
        },
        async handleDecision(): Promise<void> {},
        getStatus(): ApprovalStatus | null {
          return null;
        },
        async cancelRequest(): Promise<boolean> {
          return false;
        },
        async updatePolicy(): Promise<void> {},
        getPolicy(): ApprovalPolicy {
          return { mode: 'always_ask' };
        },
      };

      const request: ApprovalRequest = {
        id: 'approval_timeout',
        type: 'command',
        title: 'Long Running Command',
        description: 'This command requires user approval',
        details: {
          command: 'long-running-process',
          riskLevel: 'medium',
        },
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_6',
          toolName: 'shell_exec',
          timestamp: Date.now(),
        },
        timeout: 50, // Short timeout for testing
      };

      const response = await mockApprovalManager.requestApproval(request);

      expect(response.decision).toBe('reject');
      expect(response.reason).toContain('timed out');
      expect(response.metadata?.timeout).toBe(true);

      const timeoutEvent = eventCollector.findByType('ApprovalTimeout');
      expect(timeoutEvent).toBeDefined();
    });

    it('should support approval cancellation', async () => {
      let pendingRequests = new Set(['approval_cancel']);

      const mockApprovalManager: ApprovalManager = {
        async requestApproval(request: ApprovalRequest): Promise<ApprovalResponse> {
          pendingRequests.add(request.id);

          eventCollector.collect({
            id: 'evt_approval_requested',
            msg: {
              type: 'ApprovalRequested',
              data: {
                approval_id: request.id,
              },
            },
          });

          // Wait for cancellation or user decision
          return new Promise((resolve) => {
            const checkInterval = setInterval(() => {
              if (!pendingRequests.has(request.id)) {
                clearInterval(checkInterval);
                resolve({
                  id: request.id,
                  decision: 'reject',
                  timestamp: Date.now(),
                  reason: 'Request was canceled',
                  metadata: { canceled: true },
                });
              }
            }, 10);
          });
        },
        async handleDecision(): Promise<void> {},
        getStatus(id: string): ApprovalStatus | null {
          return pendingRequests.has(id) ? {
            id,
            status: 'pending',
            timestamp: Date.now(),
          } : null;
        },
        async cancelRequest(id: string): Promise<boolean> {
          if (!pendingRequests.has(id)) {
            return false;
          }

          pendingRequests.delete(id);

          eventCollector.collect({
            id: 'evt_approval_canceled',
            msg: {
              type: 'ApprovalCanceled',
              data: {
                approval_id: id,
                reason: 'User canceled request',
              },
            },
          });

          return true;
        },
        async updatePolicy(): Promise<void> {},
        getPolicy(): ApprovalPolicy {
          return { mode: 'always_ask' };
        },
      };

      const request: ApprovalRequest = {
        id: 'approval_cancel',
        type: 'command',
        title: 'Cancelable Command',
        description: 'This command can be canceled',
        details: {
          command: 'cancelable-process',
          riskLevel: 'low',
        },
        metadata: {
          sessionId: 'session_1',
          turnId: 'turn_7',
          toolName: 'shell_exec',
          timestamp: Date.now(),
        },
      };

      // Start approval request
      const approvalPromise = mockApprovalManager.requestApproval(request);

      // Wait for request to be registered
      await waitFor(() => mockApprovalManager.getStatus('approval_cancel') !== null);

      // Cancel the request
      const canceled = await mockApprovalManager.cancelRequest('approval_cancel');
      expect(canceled).toBe(true);

      // Wait for the approval to complete
      const response = await approvalPromise;

      expect(response.decision).toBe('reject');
      expect(response.reason).toContain('canceled');
      expect(response.metadata?.canceled).toBe(true);

      const cancelEvent = eventCollector.findByType('ApprovalCanceled');
      expect(cancelEvent).toBeDefined();

      // Verify status is cleared
      const status = mockApprovalManager.getStatus('approval_cancel');
      expect(status).toBeNull();
    });
  });

  describe('Status Tracking', () => {
    it('should track approval status', () => {
      const mockApprovalManager: ApprovalManager = {
        async requestApproval(): Promise<ApprovalResponse> {
          return {
            id: 'test',
            decision: 'approve',
            timestamp: Date.now(),
          };
        },
        async handleDecision(): Promise<void> {},
        getStatus(id: string): ApprovalStatus | null {
          const mockStatuses: Record<string, ApprovalStatus> = {
            'pending_approval': {
              id: 'pending_approval',
              status: 'pending',
              timestamp: Date.now() - 5000,
              timeRemaining: 25000,
              policy: { mode: 'always_ask', timeout: 30000 },
            },
            'approved_request': {
              id: 'approved_request',
              status: 'approved',
              decision: 'approve',
              timestamp: Date.now() - 10000,
            },
            'rejected_request': {
              id: 'rejected_request',
              status: 'rejected',
              decision: 'reject',
              timestamp: Date.now() - 15000,
            },
          };

          return mockStatuses[id] || null;
        },
        async cancelRequest(): Promise<boolean> {
          return false;
        },
        async updatePolicy(): Promise<void> {},
        getPolicy(): ApprovalPolicy {
          return { mode: 'always_ask' };
        },
      };

      // Test pending status
      const pendingStatus = mockApprovalManager.getStatus('pending_approval');
      expect(pendingStatus).toMatchObject({
        id: 'pending_approval',
        status: 'pending',
        timestamp: expect.any(Number),
        timeRemaining: expect.any(Number),
        policy: expect.objectContaining({
          mode: 'always_ask',
        }),
      });

      // Test approved status
      const approvedStatus = mockApprovalManager.getStatus('approved_request');
      expect(approvedStatus?.status).toBe('approved');
      expect(approvedStatus?.decision).toBe('approve');

      // Test rejected status
      const rejectedStatus = mockApprovalManager.getStatus('rejected_request');
      expect(rejectedStatus?.status).toBe('rejected');
      expect(rejectedStatus?.decision).toBe('reject');

      // Test non-existent status
      const nonExistentStatus = mockApprovalManager.getStatus('nonexistent');
      expect(nonExistentStatus).toBeNull();
    });
  });
});