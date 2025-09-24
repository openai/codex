/**
 * ApprovalManager - Handles approval requests with policies and timeout handling
 * Based on contract from approval-manager.test.ts
 */

import type { ReviewDecision } from '../protocol/types';
import type { Event } from '../protocol/types';

export interface ApprovalRequest {
  id: string;
  type: 'command' | 'file_operation' | 'network_access' | 'storage_access' | 'dangerous_action';
  title: string;
  description: string;
  details: ApprovalDetails;
  metadata?: ApprovalMetadata;
  timeout?: number;
  policy?: ApprovalPolicy;
}

export interface ApprovalDetails {
  command?: string;
  filePath?: string;
  url?: string;
  action?: string;
  parameters?: Record<string, any>;
  riskLevel: 'low' | 'medium' | 'high' | 'critical';
  impact?: string[];
}

export interface ApprovalMetadata {
  sessionId: string;
  turnId: string;
  toolName: string;
  timestamp: number;
  userId?: string;
  rollbackable: boolean;
  description?: string;
  tags?: string[];
}

export interface ApprovalResponse {
  id: string;
  decision: ReviewDecision;
  timestamp: number;
  reason?: string;
  modifications?: Record<string, any>;
  metadata?: Record<string, any>;
}

export interface ApprovalPolicy {
  mode: 'always_ask' | 'auto_approve_safe' | 'auto_reject_unsafe' | 'never_ask';
  riskThreshold?: 'low' | 'medium' | 'high';
  trustedDomains?: string[];
  allowedCommands?: string[];
  blockedCommands?: string[];
  timeout?: number;
}

export interface ApprovalStatus {
  id: string;
  status: 'pending' | 'approved' | 'rejected' | 'timeout' | 'canceled';
  decision?: ReviewDecision;
  timestamp: number;
  timeRemaining?: number;
  policy?: ApprovalPolicy;
}

/**
 * ApprovalManager implementation
 */
export class ApprovalManager {
  private policy: ApprovalPolicy = { mode: 'always_ask' };
  private pendingRequests = new Map<string, PendingApproval>();
  private approvalHistory = new Map<string, ApprovalResponse>();
  private eventEmitter?: (event: Event) => void;

  constructor(eventEmitter?: (event: Event) => void) {
    this.eventEmitter = eventEmitter;
  }

  /**
   * Request approval for an action
   */
  async requestApproval(request: ApprovalRequest): Promise<ApprovalResponse> {
    // Apply policy to determine if we should auto-approve/reject
    const policyDecision = this.evaluatePolicy(request);
    if (policyDecision) {
      return policyDecision;
    }

    // Emit approval requested event
    this.emitEvent({
      id: `evt_approval_requested_${request.id}`,
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

    // Set up timeout handling
    const timeout = request.timeout || this.policy.timeout || 30000;
    const timeoutPromise = new Promise<ApprovalResponse>((resolve) => {
      setTimeout(() => {
        this.pendingRequests.delete(request.id);

        this.emitEvent({
          id: `evt_approval_timeout_${request.id}`,
          msg: {
            type: 'ApprovalTimeout',
            data: {
              approval_id: request.id,
              timeout_ms: timeout,
            },
          },
        });

        const timeoutResponse: ApprovalResponse = {
          id: request.id,
          decision: 'reject',
          timestamp: Date.now(),
          reason: 'Request timed out',
          metadata: { timeout: true },
        };

        this.approvalHistory.set(request.id, timeoutResponse);
        resolve(timeoutResponse);
      }, timeout);
    });

    // Create pending approval entry
    const pendingApproval: PendingApproval = {
      request,
      timestamp: Date.now(),
      timeRemaining: timeout,
    };

    this.pendingRequests.set(request.id, pendingApproval);

    // Wait for user decision or timeout
    const userDecisionPromise = new Promise<ApprovalResponse>((resolve) => {
      pendingApproval.resolver = resolve;
    });

    return Promise.race([userDecisionPromise, timeoutPromise]);
  }

  /**
   * Handle approval decision from user
   */
  async handleDecision(response: ApprovalResponse): Promise<void> {
    const pending = this.pendingRequests.get(response.id);
    if (!pending) {
      return; // Request already processed or doesn't exist
    }

    // Remove from pending
    this.pendingRequests.delete(response.id);

    // Store in history
    this.approvalHistory.set(response.id, response);

    // Emit appropriate event
    const eventType = response.decision === 'approve' ? 'ApprovalGranted' :
                     response.decision === 'reject' ? 'ApprovalRejected' :
                     'ApprovalModified';

    this.emitEvent({
      id: `evt_approval_${response.decision}_${response.id}`,
      msg: {
        type: eventType as any,
        data: {
          approval_id: response.id,
          decision: response.decision,
          reason: response.reason,
          modifications: response.modifications,
        },
      },
    });

    // Resolve the pending promise
    if (pending.resolver) {
      pending.resolver(response);
    }
  }

  /**
   * Get approval status
   */
  getStatus(id: string): ApprovalStatus | null {
    const pending = this.pendingRequests.get(id);
    if (pending) {
      const elapsed = Date.now() - pending.timestamp;
      return {
        id,
        status: 'pending',
        timestamp: pending.timestamp,
        timeRemaining: Math.max(0, pending.timeRemaining - elapsed),
        policy: this.policy,
      };
    }

    const history = this.approvalHistory.get(id);
    if (history) {
      return {
        id,
        status: history.decision === 'approve' ? 'approved' : 'rejected',
        decision: history.decision,
        timestamp: history.timestamp,
      };
    }

    return null;
  }

  /**
   * Cancel pending approval request
   */
  async cancelRequest(id: string): Promise<boolean> {
    const pending = this.pendingRequests.get(id);
    if (!pending) {
      return false;
    }

    this.pendingRequests.delete(id);

    this.emitEvent({
      id: `evt_approval_canceled_${id}`,
      msg: {
        type: 'ApprovalCanceled',
        data: {
          approval_id: id,
          reason: 'User canceled request',
        },
      },
    });

    // Resolve with canceled response
    if (pending.resolver) {
      const canceledResponse: ApprovalResponse = {
        id,
        decision: 'reject',
        timestamp: Date.now(),
        reason: 'Request was canceled',
        metadata: { canceled: true },
      };

      this.approvalHistory.set(id, canceledResponse);
      pending.resolver(canceledResponse);
    }

    return true;
  }

  /**
   * Update approval policy
   */
  async updatePolicy(updates: Partial<ApprovalPolicy>): Promise<void> {
    const oldPolicy = { ...this.policy };
    this.policy = { ...this.policy, ...updates };

    this.emitEvent({
      id: `evt_policy_updated_${Date.now()}`,
      msg: {
        type: 'PolicyUpdated',
        data: {
          policy: this.policy,
          changes: updates,
        },
      },
    });
  }

  /**
   * Get current policy
   */
  getPolicy(): ApprovalPolicy {
    return { ...this.policy };
  }

  /**
   * Get approval history
   */
  getApprovalHistory(): ApprovalResponse[] {
    return Array.from(this.approvalHistory.values());
  }

  /**
   * Clear approval history
   */
  clearHistory(): void {
    this.approvalHistory.clear();
  }

  /**
   * Get pending approvals
   */
  getPendingApprovals(): ApprovalRequest[] {
    return Array.from(this.pendingRequests.values()).map(p => p.request);
  }

  /**
   * Evaluate policy for automatic decisions
   */
  private evaluatePolicy(request: ApprovalRequest): ApprovalResponse | null {
    const { mode, riskThreshold, allowedCommands, blockedCommands, trustedDomains } = this.policy;

    // Never ask mode - auto approve everything (dangerous!)
    if (mode === 'never_ask') {
      return this.createAutoResponse(request, 'approve', 'Auto-approved by never_ask policy');
    }

    // Auto approve safe actions
    if (mode === 'auto_approve_safe') {
      const isLowRisk = request.details.riskLevel === 'low';
      const isAllowedCommand = !request.details.command ||
        (allowedCommands && allowedCommands.some(cmd =>
          request.details.command!.startsWith(cmd)
        ));
      const isTrustedDomain = !request.details.url ||
        (trustedDomains && trustedDomains.some(domain =>
          this.matchesDomain(request.details.url!, domain)
        ));

      if (isLowRisk && isAllowedCommand && isTrustedDomain) {
        return this.createAutoResponse(request, 'approve', 'Auto-approved by policy', { autoApproved: true });
      }
    }

    // Auto reject unsafe actions
    if (mode === 'auto_reject_unsafe') {
      const isHighRisk = request.details.riskLevel === 'high' || request.details.riskLevel === 'critical';
      const isBlockedCommand = request.details.command &&
        blockedCommands &&
        blockedCommands.some(cmd => request.details.command!.includes(cmd));
      const exceedsThreshold = riskThreshold &&
        this.riskLevelExceeds(request.details.riskLevel, riskThreshold);

      if (isHighRisk || isBlockedCommand || exceedsThreshold) {
        return this.createAutoResponse(request, 'reject', 'Auto-rejected by policy', { autoRejected: true });
      }
    }

    return null; // No automatic decision, require user input
  }

  /**
   * Create automatic approval response
   */
  private createAutoResponse(
    request: ApprovalRequest,
    decision: ReviewDecision,
    reason: string,
    metadata?: Record<string, any>
  ): ApprovalResponse {
    const response: ApprovalResponse = {
      id: request.id,
      decision,
      timestamp: Date.now(),
      reason,
      metadata,
    };

    // Store in history
    this.approvalHistory.set(request.id, response);

    // Emit event
    const eventType = decision === 'approve' ? 'AutoApproved' : 'AutoRejected';
    this.emitEvent({
      id: `evt_auto_${decision}_${request.id}`,
      msg: {
        type: eventType as any,
        data: {
          approval_id: request.id,
          policy_reason: reason,
        },
      },
    });

    return response;
  }

  /**
   * Check if URL matches domain pattern
   */
  private matchesDomain(url: string, pattern: string): boolean {
    try {
      const urlObj = new URL(url);
      const hostname = urlObj.hostname;

      if (pattern.startsWith('*.')) {
        const domain = pattern.slice(2);
        return hostname === domain || hostname.endsWith('.' + domain);
      }

      return hostname === pattern;
    } catch {
      return false;
    }
  }

  /**
   * Check if risk level exceeds threshold
   */
  private riskLevelExceeds(level: string, threshold: string): boolean {
    const levels = ['low', 'medium', 'high', 'critical'];
    const levelIndex = levels.indexOf(level);
    const thresholdIndex = levels.indexOf(threshold);
    return levelIndex > thresholdIndex;
  }

  /**
   * Emit event if emitter is available
   */
  private emitEvent(event: Event): void {
    if (this.eventEmitter) {
      this.eventEmitter(event);
    }
  }
}

/**
 * Internal pending approval tracking
 */
interface PendingApproval {
  request: ApprovalRequest;
  timestamp: number;
  timeRemaining: number;
  resolver?: (response: ApprovalResponse) => void;
}