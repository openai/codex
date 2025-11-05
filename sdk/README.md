# @codex/orchestrator

ClaudeCode-style autonomous sub-agent orchestration for Codex.

## 🚀 Features

- **Automatic Task Analysis**: Analyzes task complexity and determines if orchestration would benefit
- **Transparent Orchestration**: Automatically delegates to specialized sub-agents when needed
- **Multiple Strategies**: Sequential, parallel, or hybrid execution
- **MCP Protocol**: Uses Model Context Protocol for secure Rust ↔ Node.js integration
- **Streaming Support**: Real-time progress updates via event streams

## 📦 Installation

```bash
npm install @codex/orchestrator
```

## 🎯 Quick Start

```typescript
import { CodexOrchestrator } from '@codex/orchestrator';

const orchestrator = new CodexOrchestrator();

// Execute with auto-orchestration
const result = await orchestrator.execute(
  "Implement user authentication with JWT, write tests, and security review"
);

console.log(`Orchestrated: ${result.wasOrchestrated}`);
console.log(`Agents used: ${result.agentsUsed.join(', ')}`);
console.log(result.executionSummary);

// Cleanup
await orchestrator.close();
```

## 📚 API Reference

### `CodexOrchestrator`

Main class for autonomous orchestration.

#### Constructor

```typescript
new CodexOrchestrator(codexCommand?: string)
```

- `codexCommand`: Path to codex binary (default: 'codex')

#### Methods

##### `execute(goal, options?)`

Execute a task with automatic orchestration.

```typescript
async execute(
  goal: string,
  options?: OrchestrateOptions
): Promise<OrchestratedResult>
```

**Parameters**:
- `goal`: The task goal to execute
- `options.complexityThreshold`: Threshold for triggering orchestration (0.0-1.0, default: 0.7)
- `options.strategy`: Execution strategy ('sequential' | 'parallel' | 'hybrid', default: 'hybrid')
- `options.format`: Output format ('text' | 'json', default: 'json')

**Returns**: `OrchestratedResult` with execution details

##### `executeStream(goal, options?)`

Execute with streaming progress updates.

```typescript
async *executeStream(
  goal: string,
  options?: OrchestrateOptions
): AsyncIterableIterator<OrchestrationEvent>
```

**Yields**: `OrchestrationEvent` objects with progress updates

##### `close()`

Close MCP connection and cleanup resources.

```typescript
async close(): Promise<void>
```

### Types

#### `OrchestratedResult`

```typescript
interface OrchestratedResult {
  wasOrchestrated: boolean;
  agentsUsed: string[];
  executionSummary: string;
  agentResults?: AgentResult[];
  totalExecutionTimeSecs?: number;
  taskAnalysis?: TaskAnalysis;
}
```

#### `OrchestrateOptions`

```typescript
interface OrchestrateOptions {
  complexityThreshold?: number;  // 0.0-1.0
  strategy?: 'sequential' | 'parallel' | 'hybrid';
  format?: 'text' | 'json';
}
```

## 🎨 Usage Examples

### Basic Usage

```typescript
const orchestrator = new CodexOrchestrator();

const result = await orchestrator.execute(
  "Refactor authentication module"
);

if (result.wasOrchestrated) {
  console.log('✅ Orchestrated with agents:', result.agentsUsed);
} else {
  console.log('ℹ️  Normal execution');
}

await orchestrator.close();
```

### Custom Threshold

```typescript
// Higher threshold = less likely to orchestrate
const result = await orchestrator.execute(
  "Fix typo",
  { complexityThreshold: 0.9 }
);
```

### Sequential Execution

```typescript
// Execute agents one by one (not in parallel)
const result = await orchestrator.execute(
  "Migrate database schema and update API",
  { strategy: 'sequential' }
);
```

### Streaming Progress

```typescript
for await (const event of orchestrator.executeStream("Build full-stack app")) {
  console.log(`[${event.type}] ${event.message}`);
}
```

### Error Handling

```typescript
try {
  const result = await orchestrator.execute("Complex task");
  console.log(result.executionSummary);
} catch (error) {
  console.error('Orchestration failed:', error);
} finally {
  await orchestrator.close();
}
```

## 🔧 Requirements

- Node.js >= 22
- Codex CLI installed and in PATH
- Codex configured with agent definitions in `.codex/agents/`

## 🧪 Testing

```bash
npm test
```

Note: Integration tests require a running Codex instance.

## 📝 License

MIT

## 🔗 Related

- [Codex Documentation](https://github.com/openai/codex)
- [MCP Protocol](https://modelcontextprotocol.io)
- [Agent Definitions](./.codex/agents/)
