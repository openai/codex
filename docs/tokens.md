# Token Budget Management

## Overview

Codex includes a token budget tracking system to monitor and limit AI model token usage across agents and operations. This helps manage costs and prevent unexpected API expenses.

## Configuration

### Configuration File (`.codex/config.toml`)

```toml
[token_budget]
# Total token budget (0 = unlimited)
total_budget = 1000000

# Warning threshold (percentage: 0-100)
warning_threshold = 80

# Per-agent token limits
[token_budget.per_agent_limits]
code-reviewer = 100000
test-gen = 50000
sec-audit = 75000
```

### Environment Variables

Override configuration file settings:

```bash
# Total token budget
export CODEX_TOKEN_BUDGET=1000000

# Warning threshold
export CODEX_TOKEN_WARNING_THRESHOLD=80
```

## Usage Tracking

The token budget tracker automatically records usage for each:
- Agent invocation
- Model API call (prompt + completion tokens)
- Timestamp of usage

### Example Usage Data

```json
{
  "agent_id": "code-reviewer",
  "model": "gpt-4",
  "prompt_tokens": 1500,
  "completion_tokens": 800,
  "total_tokens": 2300,
  "timestamp": 1698765432
}
```

## Budget Enforcement

### Total Budget

When total budget is set (non-zero):
- Token usage is tracked across all agents
- New requests are rejected if they would exceed the budget
- Warning is emitted when usage crosses threshold

### Per-Agent Limits

Configure limits for specific agents:

```toml
[token_budget.per_agent_limits]
expensive-agent = 50000  # This agent limited to 50K tokens
```

If an agent exceeds its limit:
```
Error: Agent expensive-agent would exceed limit of 50000 tokens 
(current: 48000, requested: 3000)
```

## Warning Thresholds

When token usage crosses the warning threshold (default 80%):

```
WARN: Token budget warning: 81.2% used (812000 / 1000000)
```

This warning is emitted once per session when threshold is crossed.

## Status Reporting

### Programmatic Status

```rust
use codex_core::token_budget::TokenBudgetTracker;

let tracker = TokenBudgetTracker::with_defaults();
let status = tracker.get_status()?;

println!("Total used: {}", status.total_used);
println!("Usage: {:.1}%", status.usage_percentage.unwrap_or(0.0));

for (agent, tokens) in status.agent_totals {
    println!("  {}: {} tokens", agent, tokens);
}
```

### CLI Status (Planned)

```bash
# View current token usage
codex tokens status

# Reset token tracking
codex tokens reset
```

## Best Practices

### 1. Set Realistic Budgets

Base budgets on your API tier and expected usage:

```toml
[token_budget]
# Example: $10/month budget @ $0.01/1K tokens = 1M tokens
total_budget = 1000000
```

### 2. Monitor Per-Agent Usage

Track which agents consume the most tokens:

```toml
[token_budget.per_agent_limits]
# Expensive operations get lower limits
deep-research = 100000
code-reviewer = 200000

# Lightweight operations get higher limits  
linter = 500000
formatter = 500000
```

### 3. Use Warning Thresholds

Set threshold to give advance notice:

```toml
[token_budget]
warning_threshold = 75  # Warning at 75% usage
```

### 4. Reset Periodically

Reset counters at billing cycle boundaries:

```rust
// In automation or cron job
tracker.reset()?;
```

## Integration with Orchestrator

When using the orchestrator (planned):
- Token usage is reported via `tokens.reportUsage` RPC
- Status updates broadcast via `tokens.updated` events
- GUI displays real-time usage metrics

### RPC Example

```typescript
// Report token usage
await client.call('tokens.reportUsage', {
  agent_id: 'code-reviewer',
  model: 'gpt-4',
  prompt_tokens: 1500,
  completion_tokens: 800
});

// Get current budget status
const status = await client.call('tokens.getBudget', {});
console.log('Remaining:', status.remaining_budget);
```

### Event Subscription

```typescript
// Subscribe to token updates
client.subscribe(['tokens.updated'], (event) => {
  console.log('Token usage:', event.data.total_used);
  console.log('Percentage:', event.data.usage_percentage);
});
```

## Cost Estimation

### Token-to-Cost Conversion

Estimate costs based on model pricing:

```python
# GPT-4 pricing example
PROMPT_COST_PER_1K = 0.03  # $0.03/1K tokens
COMPLETION_COST_PER_1K = 0.06  # $0.06/1K tokens

prompt_cost = (prompt_tokens / 1000) * PROMPT_COST_PER_1K
completion_cost = (completion_tokens / 1000) * COMPLETION_COST_PER_1K
total_cost = prompt_cost + completion_cost
```

### Budget-to-Cost

```python
# Convert token budget to dollar amount
TOKEN_BUDGET = 1000000  # 1M tokens
AVG_COST_PER_1K = 0.04  # Average across prompt/completion

estimated_cost = (TOKEN_BUDGET / 1000) * AVG_COST_PER_1K
# $40 for 1M tokens
```

## Troubleshooting

### Budget Exceeded Errors

```
Error: Total budget of 1000000 tokens would be exceeded 
(current: 995000, requested: 10000)
```

**Solutions**:
1. Increase budget in config
2. Reset usage counters if at billing cycle
3. Optimize prompts to reduce token usage
4. Use cheaper models for non-critical tasks

### Per-Agent Limit Exceeded

```
Error: Agent code-reviewer would exceed limit of 100000 tokens 
(current: 98000, requested: 5000)
```

**Solutions**:
1. Increase agent-specific limit
2. Reduce scope of agent operations
3. Split work across multiple agents

### Inaccurate Tracking

Token counts may differ from API billing due to:
- System messages (not tracked)
- Function call tokens
- Model-specific tokenization differences

**Mitigation**: Add 10-15% buffer to budgets

---

**日本語版 / Japanese Version**

## 概要

Codexは、エージェントと操作全体でAIモデルのトークン使用量を監視および制限するトークンバジェット追跡システムを含んでいます。これによりコストを管理し、予期しないAPI支出を防ぎます。

## 設定

### 設定ファイル (`.codex/config.toml`)

```toml
[token_budget]
# 総トークンバジェット（0 = 無制限）
total_budget = 1000000

# 警告閾値（パーセンテージ: 0-100）
warning_threshold = 80

# エージェント別トークン制限
[token_budget.per_agent_limits]
code-reviewer = 100000
test-gen = 50000
sec-audit = 75000
```

## バジェット適用

### 総バジェット

総バジェットが設定されている場合（ゼロ以外）：
- 全エージェントでトークン使用量が追跡される
- バジェットを超える新しいリクエストは拒否される
- 使用量が閾値を超えると警告が出される

### エージェント別制限

特定のエージェントに制限を設定：

```toml
[token_budget.per_agent_limits]
expensive-agent = 50000  # このエージェントは5万トークンに制限
```

## ベストプラクティス

### 1. 現実的なバジェットを設定

APIティアと予想される使用量に基づいてバジットを設定：

```toml
[token_budget]
# 例: 月額$10のバジット @ $0.01/1Kトークン = 100万トークン
total_budget = 1000000
```

### 2. エージェント別使用量を監視

どのエージェントが最もトークンを消費するか追跡。

### 3. 警告閾値を使用

事前に通知を受けるために閾値を設定：

```toml
[token_budget]
warning_threshold = 75  # 75%使用時に警告
```
