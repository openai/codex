/**
 * React hooks for Orchestrator Client
 */

import { useEffect, useState, useCallback, useRef } from 'react';
import { OrchestratorClient, OrchestratorClientConfig } from './client';
import type * as Types from './types';

/**
 * useProtocol hook
 * 
 * Provides a singleton Orchestrator client instance
 */
export function useProtocol(config?: OrchestratorClientConfig): OrchestratorClient {
  const clientRef = useRef<OrchestratorClient | null>(null);

  if (!clientRef.current) {
    clientRef.current = new OrchestratorClient(config);
  }

  const client = clientRef.current;

  useEffect(() => {
    let mounted = true;

    client.connect().catch((error: any) => {
      if (mounted) {
        console.error('Failed to connect:', error);
      }
    });

    return () => {
      mounted = false;
      client.disconnect();
    };
  }, [client]);

  return client;
}

/**
 * useProtocolEvent hook
 * 
 * Subscribe to RPC events
 */
export function useProtocolEvent(
  client: OrchestratorClient,
  topic: string
): Types.RpcEvent | null {
  const [event, setEvent] = useState<Types.RpcEvent | null>(null);

  useEffect(() => {
    const handler = (e: Types.RpcEvent) => {
      if (e.topic === topic) {
        setEvent(e);
      }
    };

    (client as any).on('event', handler);

    return () => {
      (client as any).off('event', handler);
    };
  }, [client, topic]);

  return event;
}

/**
 * useOrchestratorStatus hook
 * 
 * Poll orchestrator status
 */
export function useOrchestratorStatus(
  client: OrchestratorClient,
  pollInterval: number = 5000
): {
  status: Types.StatusGetResponse | null;
  loading: boolean;
  error: Error | null;
  refresh: () => Promise<void>;
} {
  const [status, setStatus] = useState<Types.StatusGetResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const fetchStatus = useCallback(async () => {
    try {
      setLoading(true);
      const result = await client.statusGet();
      setStatus(result);
      setError(null);
    } catch (err) {
      setError(err as Error);
    } finally {
      setLoading(false);
    }
  }, [client]);

  useEffect(() => {
    fetchStatus();

    const interval = setInterval(fetchStatus, pollInterval);

    return () => {
      clearInterval(interval);
    };
  }, [fetchStatus, pollInterval]);

  return { status, loading, error, refresh: fetchStatus };
}

/**
 * useLockStatus hook
 * 
 * Monitor lock status with real-time updates
 */
export function useLockStatus(
  client: OrchestratorClient,
  path?: string
): {
  status: Types.LockStatusResponse | null;
  loading: boolean;
  error: Error | null;
  acquire: (force?: boolean) => Promise<void>;
  release: () => Promise<void>;
} {
  const [status, setStatus] = useState<Types.LockStatusResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const fetchStatus = useCallback(async () => {
    try {
      setLoading(true);
      const result = await client.lockStatus({ path });
      setStatus(result);
      setError(null);
    } catch (err) {
      setError(err as Error);
    } finally {
      setLoading(false);
    }
  }, [client, path]);

  useEffect(() => {
    fetchStatus();

    // Subscribe to lock.changed events
    const handler = (e: Types.RpcEvent) => {
      if (e.topic === 'lock.changed') {
        fetchStatus();
      }
    };

    (client as any).on('event', handler);

    return () => {
      (client as any).off('event', handler);
    };
  }, [client, fetchStatus]);

  const acquire = useCallback(async (force = false) => {
    if (!path) {
      throw new Error('Path is required for lock.acquire');
    }

    try {
      await client.lockAcquire({ path, force });
      await fetchStatus();
    } catch (err) {
      setError(err as Error);
      throw err;
    }
  }, [client, path, fetchStatus]);

  const release = useCallback(async () => {
    if (!path) {
      throw new Error('Path is required for lock.release');
    }

    try {
      await client.lockRelease({ path });
      await fetchStatus();
    } catch (err) {
      setError(err as Error);
      throw err;
    }
  }, [client, path, fetchStatus]);

  return { status, loading, error, acquire, release };
}

/**
 * useTokenBudget hook
 * 
 * Monitor token budget with real-time updates
 */
export function useTokenBudget(
  client: OrchestratorClient
): {
  budget: Types.TokensGetBudgetResponse | null;
  loading: boolean;
  error: Error | null;
  refresh: () => Promise<void>;
} {
  const [budget, setBudget] = useState<Types.TokensGetBudgetResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const fetchBudget = useCallback(async () => {
    try {
      setLoading(true);
      const result = await client.tokensGetBudget();
      setBudget(result);
      setError(null);
    } catch (err) {
      setError(err as Error);
    } finally {
      setLoading(false);
    }
  }, [client]);

  useEffect(() => {
    fetchBudget();

    // Subscribe to tokens.updated events
    const handler = (e: Types.RpcEvent) => {
      if (e.topic === 'tokens.updated') {
        fetchBudget();
      }
    };

    (client as any).on('event', handler);

    return () => {
      (client as any).off('event', handler);
    };
  }, [client, fetchBudget]);

  return { budget, loading, error, refresh: fetchBudget };
}

/**
 * useAgentList hook
 * 
 * Monitor active agents
 */
export function useAgentList(
  client: OrchestratorClient,
  pollInterval: number = 10000
): {
  agents: Types.AgentInfo[];
  loading: boolean;
  error: Error | null;
  refresh: () => Promise<void>;
} {
  const [agents, setAgents] = useState<Types.AgentInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const fetchAgents = useCallback(async () => {
    try {
      setLoading(true);
      const result = await client.agentList();
      setAgents(result.agents);
      setError(null);
    } catch (err) {
      setError(err as Error);
    } finally {
      setLoading(false);
    }
  }, [client]);

  useEffect(() => {
    fetchAgents();

    const interval = setInterval(fetchAgents, pollInterval);

    // Subscribe to agent.status events
    const handler = (e: Types.RpcEvent) => {
      if (e.topic === 'agent.status') {
        fetchAgents();
      }
    };

    (client as any).on('event', handler);

    return () => {
      clearInterval(interval);
      (client as any).off('event', handler);
    };
  }, [client, fetchAgents, pollInterval]);

  return { agents, loading, error, refresh: fetchAgents };
}

