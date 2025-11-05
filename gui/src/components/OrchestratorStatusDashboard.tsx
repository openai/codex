/**
 * Orchestrator Status Dashboard
 * Real-time monitoring of orchestrator server status
 */

'use client';

import React from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Progress } from '@/components/ui/progress';
import {
  Activity,
  Users,
  List Checked,
  Coins,
  Server,
  Clock,
  AlertCircle,
} from 'lucide-react';
import { useOrchestratorStatus, useTokenBudget, useAgentList, useLockStatus } from '@zapabob/codex-protocol-client/react';
import { OrchestratorClient } from '@zapabob/codex-protocol-client';

interface OrchestratorStatusDashboardProps {
  client: OrchestratorClient;
  className?: string;
}

function formatUptime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  } else if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  } else {
    return `${secs}s`;
  }
}

function formatTokens(tokens: number): string {
  if (tokens >= 1000000) {
    return `${(tokens / 1000000).toFixed(1)}M`;
  } else if (tokens >= 1000) {
    return `${(tokens / 1000).toFixed(1)}K`;
  } else {
    return tokens.toString();
  }
}

export function OrchestratorStatusDashboard({ client, className }: OrchestratorStatusDashboardProps) {
  const { status, loading: statusLoading, error: statusError } = useOrchestratorStatus(client, 5000);
  const { budget, loading: budgetLoading } = useTokenBudget(client);
  const { agents, loading: agentsLoading } = useAgentList(client, 10000);
  const { status: lockStatus } = useLockStatus(client);

  if (statusError) {
    return (
      <Card className={className}>
        <CardContent className="pt-6">
          <div className="flex items-center gap-2 text-destructive">
            <AlertCircle className="h-4 w-4" />
            <span>Failed to connect to Orchestrator: {statusError.message}</span>
          </div>
        </CardContent>
      </Card>
    );
  }

  if (statusLoading || !status) {
    return (
      <Card className={className}>
        <CardContent className="pt-6">
          <div className="flex items-center gap-2 text-muted-foreground">
            <Activity className="h-4 w-4 animate-pulse" />
            <span>Connecting to Orchestrator...</span>
          </div>
        </CardContent>
      </Card>
    );
  }

  const tokenUsagePercent = budget ? (budget.used / budget.total_budget) * 100 : 0;
  const isTokenWarning = budget && budget.used >= budget.warning_threshold;
  const queueUsagePercent = status ? (status.queue_size / status.queue_capacity) * 100 : 0;

  return (
    <div className={`grid gap-4 md:grid-cols-2 lg:grid-cols-4 ${className}`}>
      {/* Server Status */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-sm font-medium">Server Status</CardTitle>
          <Server className="h-4 w-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-2">
            <Badge variant="default" className="bg-green-500">
              Online
            </Badge>
            <span className="text-xs text-muted-foreground">v{status.server_version}</span>
          </div>
          <div className="mt-2 flex items-center gap-1 text-xs text-muted-foreground">
            <Clock className="h-3 w-3" />
            <span>Uptime: {formatUptime(status.uptime_seconds)}</span>
          </div>
        </CardContent>
      </Card>

      {/* Active Agents */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-sm font-medium">Active Agents</CardTitle>
          <Users className="h-4 w-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">{status.active_agents}</div>
          {!agentsLoading && agents.length > 0 && (
            <div className="mt-2 space-y-1">
              {agents.slice(0, 3).map((agent) => (
                <div key={agent.agent_id} className="text-xs text-muted-foreground truncate">
                  {agent.agent_type}: {agent.status}
                </div>
              ))}
              {agents.length > 3 && (
                <div className="text-xs text-muted-foreground">
                  +{agents.length - 3} more
                </div>
              )}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Active Tasks */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-sm font-medium">Active Tasks</CardTitle>
          <ListChecked className="h-4 w-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">{status.active_tasks}</div>
          <div className="mt-2 space-y-1">
            <div className="text-xs text-muted-foreground">
              Queue: {status.queue_size} / {status.queue_capacity}
            </div>
            <Progress value={queueUsagePercent} className="h-1" />
          </div>
        </CardContent>
      </Card>

      {/* Token Budget */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-sm font-medium">Token Budget</CardTitle>
          <Coins className="h-4 w-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          {budgetLoading || !budget ? (
            <div className="text-sm text-muted-foreground">Loading...</div>
          ) : (
            <>
              <div className="text-2xl font-bold">
                {formatTokens(budget.remaining)}
              </div>
              <div className="mt-1 text-xs text-muted-foreground">
                {formatTokens(budget.used)} / {formatTokens(budget.total_budget)} used
              </div>
              <div className="mt-2 space-y-1">
                <Progress
                  value={tokenUsagePercent}
                  className={`h-1 ${isTokenWarning ? 'bg-yellow-500' : ''}`}
                />
                {isTokenWarning && (
                  <div className="flex items-center gap-1 text-xs text-yellow-600">
                    <AlertCircle className="h-3 w-3" />
                    <span>Warning threshold reached</span>
                  </div>
                )}
              </div>
            </>
          )}
        </CardContent>
      </Card>

      {/* Lock Status */}
      <Card className="md:col-span-2">
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-sm font-medium">Repository Lock</CardTitle>
          <Activity className="h-4 w-4 text-muted-foreground" />
        </CardHeader>
        <CardContent>
          {lockStatus ? (
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Badge variant={lockStatus.locked ? 'destructive' : 'secondary'}>
                  {lockStatus.locked ? 'Locked' : 'Unlocked'}
                </Badge>
                {lockStatus.holder && (
                  <span className="text-sm text-muted-foreground">
                    Held by: {lockStatus.holder}
                  </span>
                )}
              </div>
              {lockStatus.acquired_at && (
                <div className="text-xs text-muted-foreground">
                  Acquired at: {new Date(lockStatus.acquired_at).toLocaleString()}
                </div>
              )}
            </div>
          ) : (
            <div className="text-sm text-muted-foreground">No lock information</div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

export default OrchestratorStatusDashboard;

