# @zapabob/codex-protocol-client

TypeScript client library for Codex Orchestrator RPC protocol.

## Installation

```bash
npm install @zapabob/codex-protocol-client
# or
pnpm add @zapabob/codex-protocol-client
# or
yarn add @zapabob/codex-protocol-client
```

## Usage

### Basic Client

```typescript
import { OrchestratorClient } from '@zapabob/codex-protocol-client';

const client = new OrchestratorClient({
  transport: {
    preference: 'auto',
    tcpPort: 38247,
  },
  requestTimeout: 30000,
  reconnect: true,
});

// Connect to orchestrator
await client.connect();

// Get server status
const status = await client.statusGet();
console.log('Server version:', status.server_version);

// Lock operations
await client.lockAcquire({ path: '/repo', force: false });
await client.lockRelease({ path: '/repo' });

// File operations
const content = await client.fsRead({ path: 'README.md' });
await client.fsWrite({
  path: 'README.md',
  content: 'Updated content',
  preimage_sha: 'abc123...',
});

// VCS operations
const diff = await client.vcsDiff();
await client.vcsCommit({ message: 'Update files' });

// Token budget
const budget = await client.tokensGetBudget();
console.log('Remaining tokens:', budget.remaining);

// Subscribe to events
client.on('event', (event) => {
  console.log('Event:', event.topic, event.data);
});

await client.pubsubSubscribe({ topics: ['lock.changed', 'tokens.updated'] });

// Disconnect
await client.disconnect();
```

### React Hooks

```typescript
import { useProtocol, useOrchestratorStatus, useLockStatus, useTokenBudget } from '@zapabob/codex-protocol-client/react';

function App() {
  const client = useProtocol();
  const { status, loading, error } = useOrchestratorStatus(client);
  const { budget } = useTokenBudget(client);
  const { status: lockStatus, acquire, release } = useLockStatus(client, '/repo');

  if (loading) return <div>Loading...</div>;
  if (error) return <div>Error: {error.message}</div>;

  return (
    <div>
      <h1>Orchestrator Status</h1>
      <p>Version: {status?.server_version}</p>
      <p>Active Agents: {status?.active_agents}</p>
      <p>Token Usage: {budget?.used} / {budget?.total_budget}</p>
      
      <h2>Lock Status</h2>
      <p>Locked: {lockStatus?.locked ? 'Yes' : 'No'}</p>
      {lockStatus?.holder && <p>Holder: {lockStatus.holder}</p>}
      
      <button onClick={() => acquire()}>Acquire Lock</button>
      <button onClick={() => release()}>Release Lock</button>
    </div>
  );
}
```

## API Reference

### Client Methods

#### Lock Methods
- `lockStatus(request?)`: Get lock status
- `lockAcquire(request)`: Acquire repository lock
- `lockRelease(request)`: Release repository lock

#### Status Methods
- `statusGet()`: Get orchestrator server status

#### Filesystem Methods
- `fsRead(request)`: Read file content
- `fsWrite(request)`: Write file with optional preimage SHA
- `fsPatch(request)`: Apply unified diff patch

#### VCS Methods
- `vcsDiff()`: Get git diff
- `vcsCommit(request)`: Commit changes
- `vcsPush(request)`: Push to remote

#### Agent Methods
- `agentRegister(request)`: Register agent
- `agentHeartbeat(request)`: Send agent heartbeat
- `agentList()`: List active agents

#### Task Methods
- `taskSubmit(request)`: Submit task
- `taskCancel(request)`: Cancel task

#### Token Methods
- `tokensReportUsage(request)`: Report token usage
- `tokensGetBudget()`: Get token budget

#### Session Methods
- `sessionStart(request)`: Start session
- `sessionEnd(request)`: End session

#### PubSub Methods
- `pubsubSubscribe(request)`: Subscribe to topics
- `pubsubUnsubscribe(request)`: Unsubscribe from topics

### React Hooks

- `useProtocol(config?)`: Get singleton client instance
- `useProtocolEvent(client, topic)`: Subscribe to specific event
- `useOrchestratorStatus(client, pollInterval?)`: Monitor server status
- `useLockStatus(client, path?)`: Monitor lock status
- `useTokenBudget(client)`: Monitor token budget
- `useAgentList(client, pollInterval?)`: Monitor active agents

## Events

- `lock.changed`: Lock status changed
- `tokens.updated`: Token budget updated
- `agent.status`: Agent status changed
- `task.completed`: Task completed
- `task.failed`: Task failed

## License

MIT

