/**
 * Orchestrator RPC Client
 * Type-safe client for Codex Orchestrator protocol
 */

import EventEmitter from 'eventemitter3';
import { Transport, TransportConfig, createTransport } from './transport';
import type * as Types from './types';

export interface OrchestratorClientConfig {
  transport?: TransportConfig;
  requestTimeout?: number;
  reconnect?: boolean;
  reconnectDelay?: number;
  reconnectMaxAttempts?: number;
}

export interface ClientEvents {
  connected: () => void;
  disconnected: () => void;
  error: (error: Error) => void;
  event: (event: Types.RpcEvent) => void;
}

export class OrchestratorClient extends EventEmitter<ClientEvents> {
  private transport: Transport | null = null;
  private config: Required<OrchestratorClientConfig>;
  private requestId = 0;
  private pendingRequests = new Map<string, {
    resolve: (result: unknown) => void;
    reject: (error: Error) => void;
    timeout: NodeJS.Timeout;
  }>();
  private reconnectAttempts = 0;
  private reconnectTimer: NodeJS.Timeout | null = null;

  constructor(config: OrchestratorClientConfig = {}) {
    super();
    this.config = {
      transport: config.transport ?? { preference: 'auto' },
      requestTimeout: config.requestTimeout ?? 30000,
      reconnect: config.reconnect ?? true,
      reconnectDelay: config.reconnectDelay ?? 1000,
      reconnectMaxAttempts: config.reconnectMaxAttempts ?? 5,
    };
  }

  async connect(): Promise<void> {
    if (this.transport) {
      return;
    }

    this.transport = await createTransport(this.config.transport);

    this.transport.on('message', (data) => {
      this.handleMessage(data);
    });

    this.transport.on('error', (error) => {
      this.emit('error', error);
    });

    this.transport.on('close', () => {
      this.emit('disconnected');
      this.handleDisconnect();
    });

    await this.transport.connect();
    this.emit('connected');
    this.reconnectAttempts = 0;
  }

  async disconnect(): Promise<void> {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    if (this.transport) {
      await this.transport.close();
      this.transport = null;
    }

    // Reject all pending requests
    for (const [id, pending] of this.pendingRequests) {
      clearTimeout(pending.timeout);
      pending.reject(new Error('Client disconnected'));
    }
    this.pendingRequests.clear();
  }

  isConnected(): boolean {
    return this.transport !== null && this.transport.isConnected();
  }

  async request<T, P = Record<string, unknown>>(method: string, params: P = {} as P): Promise<T> {
    if (!this.isConnected()) {
      throw new Error('Client not connected');
    }

    const id = (++this.requestId).toString();
    const request: Types.RpcRequest = {
      id,
      method,
      params: params as Record<string, unknown>,
    };

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error(`Request ${id} timed out`));
      }, this.config.requestTimeout);

      this.pendingRequests.set(id, {
        resolve: resolve as (result: unknown) => void,
        reject,
        timeout,
      });

      const data = new TextEncoder().encode(JSON.stringify(request));
      this.transport!.send(data).catch((error) => {
        this.pendingRequests.delete(id);
        clearTimeout(timeout);
        reject(error);
      });
    });
  }

  private handleMessage(data: Uint8Array): void {
    try {
      const text = new TextDecoder().decode(data);
      const response = JSON.parse(text) as Types.RpcResponse;

      // Check if this is an event
      if ('topic' in response) {
        const event = response as unknown as Types.RpcEvent;
        this.emit('event', event);
        return;
      }

      const pending = this.pendingRequests.get(response.id);
      if (!pending) {
        console.warn(`Received response for unknown request: ${response.id}`);
        return;
      }

      this.pendingRequests.delete(response.id);
      clearTimeout(pending.timeout);

      if (response.error) {
        pending.reject(new Error(`RPC Error ${response.error.code}: ${response.error.message}`));
      } else {
        pending.resolve(response.result);
      }
    } catch (error) {
      console.error('Failed to parse message:', error);
    }
  }

  private handleDisconnect(): void {
    if (!this.config.reconnect || this.reconnectAttempts >= this.config.reconnectMaxAttempts) {
      return;
    }

    this.reconnectAttempts++;
    const delay = this.config.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);

    this.reconnectTimer = setTimeout(() => {
      this.connect().catch((error) => {
        console.error('Reconnect failed:', error);
      });
    }, delay);
  }

  // ========== Lock Methods ==========

  async lockStatus(request: Types.LockStatusRequest = {}): Promise<Types.LockStatusResponse> {
    return this.request('lock.status', request);
  }

  async lockAcquire(request: Types.LockAcquireRequest): Promise<Types.LockAcquireResponse> {
    return this.request('lock.acquire', request);
  }

  async lockRelease(request: Types.LockReleaseRequest): Promise<Types.LockReleaseResponse> {
    return this.request('lock.release', request);
  }

  // ========== Status Methods ==========

  async statusGet(): Promise<Types.StatusGetResponse> {
    return this.request('status.get', {});
  }

  // ========== Filesystem Methods ==========

  async fsRead(request: Types.FsReadRequest): Promise<Types.FsReadResponse> {
    return this.request('fs.read', request);
  }

  async fsWrite(request: Types.FsWriteRequest): Promise<Types.FsWriteResponse> {
    return this.request('fs.write', request);
  }

  async fsPatch(request: Types.FsPatchRequest): Promise<Types.FsPatchResponse> {
    return this.request('fs.patch', request);
  }

  // ========== VCS Methods ==========

  async vcsDiff(): Promise<Types.VcsDiffResponse> {
    return this.request('vcs.diff', {});
  }

  async vcsCommit(request: Types.VcsCommitRequest): Promise<Types.VcsCommitResponse> {
    return this.request('vcs.commit', request);
  }

  async vcsPush(request: Types.VcsPushRequest): Promise<Types.VcsPushResponse> {
    return this.request('vcs.push', request);
  }

  // ========== Agent Methods ==========

  async agentRegister(request: Types.AgentRegisterRequest): Promise<Types.AgentRegisterResponse> {
    return this.request('agent.register', request);
  }

  async agentHeartbeat(request: Types.AgentHeartbeatRequest): Promise<Types.AgentHeartbeatResponse> {
    return this.request('agent.heartbeat', request);
  }

  async agentList(): Promise<Types.AgentListResponse> {
    return this.request('agent.list', {});
  }

  // ========== Task Methods ==========

  async taskSubmit(request: Types.TaskSubmitRequest): Promise<Types.TaskSubmitResponse> {
    return this.request('task.submit', request);
  }

  async taskCancel(request: Types.TaskCancelRequest): Promise<Types.TaskCancelResponse> {
    return this.request('task.cancel', request);
  }

  // ========== Token Methods ==========

  async tokensReportUsage(request: Types.TokensReportUsageRequest): Promise<Types.TokensReportUsageResponse> {
    return this.request('tokens.reportUsage', request);
  }

  async tokensGetBudget(): Promise<Types.TokensGetBudgetResponse> {
    return this.request('tokens.getBudget', {});
  }

  // ========== Session Methods ==========

  async sessionStart(request: Types.SessionStartRequest): Promise<Types.SessionStartResponse> {
    return this.request('session.start', request);
  }

  async sessionEnd(request: Types.SessionEndRequest): Promise<Types.SessionEndResponse> {
    return this.request('session.end', request);
  }

  // ========== PubSub Methods ==========

  async pubsubSubscribe(request: Types.PubSubSubscribeRequest): Promise<Types.PubSubSubscribeResponse> {
    return this.request('pubsub.subscribe', request);
  }

  async pubsubUnsubscribe(request: Types.PubSubUnsubscribeRequest): Promise<Types.PubSubUnsubscribeResponse> {
    return this.request('pubsub.unsubscribe', request);
  }
}

