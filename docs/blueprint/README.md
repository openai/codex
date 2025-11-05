# Blueprint Mode - User Guide

**Version**: 0.57.0  
**Status**: Production Ready

---

## 📖 Table of Contents

1. [Overview](#overview)
2. [Quick Start](#quick-start)
3. [Slash Commands](#slash-commands)
4. [GUI Controls](#gui-controls)
5. [Execution Modes](#execution-modes)
6. [Deep Research Integration](#deep-research-integration)
7. [Webhook Notifications](#webhook-notifications)
8. [Examples](#examples)

---

## Overview

**Blueprint Mode** is a read-only planning phase that lets you design and review changes before execution. It provides:

- ✅ **Approval Gates**: No side effects until explicitly approved
- ✅ **Multiple Execution Strategies**: Single, Orchestrated, or Competition
- ✅ **Deep Research**: Integrated research with citations
- ✅ **Budget Enforcement**: Token and time limits
- ✅ **Webhook Notifications**: GitHub, Slack, HTTP integrations
- ✅ **Telemetry**: Privacy-respecting event collection

---

## Quick Start

### Enable Blueprint Mode

**CLI**:
```bash
codex /blueprint on
```

**VS Code**:
- Press `Shift+Tab` (toggle)
- Or: Command Palette → `Codex: Toggle Blueprint Mode`

### Create a Blueprint

**CLI**:
```bash
codex /blueprint "Add request logging" --mode=orchestrated
```

**VS Code**:
- Command Palette → `Codex: Create Blueprint`
- Enter title and goal

### Approve & Execute

```bash
# Review the blueprint
codex /blueprint export bp-123 --format=md

# Approve
codex /approve bp-123

# Execute (now unlocked)
codex execute bp-123
```

---

## Slash Commands

### `/blueprint on|off`

Toggle blueprint mode.

```bash
codex /blueprint on   # Enter blueprint mode
codex /blueprint off  # Exit blueprint mode
```

### `/blueprint "<title>" [options]`

Create a new blueprint.

**Options**:
- `--mode=single|orchestrated|competition` (default: orchestrated)
- `--budget.tokens=<number>` (default: 100000)
- `--budget.time=<minutes>` (default: 30)

**Examples**:
```bash
# Simple feature
codex /blueprint "Add logging middleware" --mode=single

# Orchestrated refactor
codex /blueprint "Refactor auth system" --mode=orchestrated --budget.tokens=150000

# Performance competition
codex /blueprint "Optimize DB query" --mode=competition
```

### `/approve <bp-id>`

Approve a blueprint for execution.

```bash
codex /approve bp-2025-11-02T12:00:00Z_add-logging
```

### `/reject <bp-id> --reason="..."`

Reject a blueprint with reason.

```bash
codex /reject bp-123 --reason="Scope too broad, split into smaller tasks"
```

### `/blueprint export <bp-id> [options]`

Export blueprint to file.

**Options**:
- `--format=md|json|both` (default: both)
- `--path=<directory>` (default: docs/blueprints)

**Examples**:
```bash
# Export both formats
codex /blueprint export bp-123

# Markdown only
codex /blueprint export bp-123 --format=md

# Custom path
codex /blueprint export bp-123 --path=./my-blueprints
```

### `/mode <single|orchestrated|competition>`

Set execution mode.

```bash
codex /mode orchestrated
codex /mode competition
```

### `/deepresearch "<query>" [options]`

Conduct deep research (requires approval).

**Options**:
- `--depth=1|2|3` (default: 2)
- `--policy=focused|comprehensive|exploratory` (default: focused)

**Examples**:
```bash
# Quick research
codex /deepresearch "React Server Components best practices"

# Deep dive
codex /deepresearch "Rust async error handling" --depth=3 --policy=comprehensive
```

---

## GUI Controls

### Status Bar

- **Inactive**: "$(edit) Enter Blueprint Mode"
- **Drafting**: "$(edit) Blueprint: drafting"
- **Pending**: "$(clock) Blueprint: pending" (amber background)
- **Approved**: "$(check) Blueprint: approved" (green)
- **Rejected**: "$(x) Blueprint: rejected" (red background)

Click to toggle blueprint mode.

### Toolbar Buttons

Located in Blueprint panel:

1. **Enter Blueprint** - Toggle mode
2. **Approve** - Approve current blueprint
3. **Reject** - Reject with reason
4. **Export** - Export MD/JSON
5. **Mode Selector** - Switch execution strategy

### Keybindings

- `Shift+Tab` - Toggle blueprint mode (editorTextFocus)
- `Ctrl+Shift+D` - Delegate task to agent
- `Ctrl+Shift+R` - Deep research
- `Ctrl+Shift+C` - Review selected code

---

## Execution Modes

### Single Mode

**Use Case**: Simple, single-file changes

**Behavior**:
- No sub-agents
- Single LLM context
- Fast execution

**Example**:
```bash
codex /blueprint "Add docstring to function" --mode=single
```

### Orchestrated Mode (Default)

**Use Case**: Complex, multi-file changes requiring coordination

**Behavior**:
- Central planner generates task DAG
- Specialist sub-agents (Backend/Frontend/DB/Security/QA)
- Integrator merges deterministic diffs
- Tests/linters run before PR

**Example**:
```bash
codex /blueprint "Refactor auth system to JWT" --mode=orchestrated
```

**Agents Used**:
- Backend Agent: Core logic
- Database Agent: Schema changes
- Security Agent: Vulnerability review
- QA Agent: Test generation

### Competition Mode

**Use Case**: Performance optimization, algorithm selection

**Behavior**:
- Spawns 2-5 git worktrees (variants A/B/C)
- Executes identical task in parallel
- Runs tests/benchmarks/linters in each
- Auto-scores: Tests (50%) + Perf (30%) + Simplicity (20%)
- Merges winner, archives losers

**Example**:
```bash
codex /blueprint "Optimize slow DB query" --mode=competition
```

**Scoring**:
```
| Variant | Tests | Performance | Simplicity | Total | Winner |
|---------|-------|-------------|------------|-------|--------|
| A       | 100.0 | 95.2        | 92.0       | 95.6  | ✅     |
| B       | 100.0 | 98.5        | 75.0       | 92.2  |        |
| C       | 100.0 | 88.0        | 95.0       | 92.6  |        |
```

---

## Deep Research Integration

### Approval Dialog

When you request deep research, an approval dialog shows:

- **Query**: "React Server Components best practices"
- **Depth**: 2
- **Domains**: duckduckgo.com, github.com, docs.rs
- **Token Budget**: ~25,000 tokens
- **Time Budget**: ~3 minutes
- **Data Retention**: 30 days, then auto-deleted

Click **Approve** or **Reject**.

### Research Block

Results appended to blueprint:

```markdown
## Research Results

**Query**: React Server Components best practices
**Depth**: 2
**Strategy**: focused
**Confidence**: 0.89

### Sources

- [Next.js Docs](https://nextjs.org/docs/app)
  - Date: 2024-10-15
  - Finding: Use async components for data fetching
  - Confidence: 0.95

### Synthesis

React Server Components enable zero-bundle-size server-side rendering...
```

---

## Webhook Notifications

### GitHub Integration

Sends commit status to GitHub:

```json
{
  "context": "codex/blueprint",
  "state": "success",
  "description": "Blueprint bp-123 approved",
  "target_url": "https://github.com/zapabob/codex/blueprints/bp-123"
}
```

**Configuration**:
```json
{
  "codex.webhooks.github.enabled": true
}
```

### Slack Integration

Posts to Slack channel:

> ✅ **Auth System Refactor**
> Blueprint approved by reviewer!
> 
> **Artifacts**: docs/blueprints/2025-11-02_refactor-auth.md

**Configuration**:
```json
{
  "codex.webhooks.slack.enabled": true
}
```

### HTTP Generic

Posts JSON to any endpoint with HMAC signature:

**Headers**:
- `X-Codex-Signature: sha256=abc123...`
- `X-Codex-Event: blueprint.approved`

---

## Examples

See `docs/blueprints/samples/` for complete examples:

1. **simple-feature.md** - Add logging middleware (single mode)
2. **orchestrated-refactor.md** - JWT auth migration (orchestrated)
3. **competition-optimization.md** - DB query optimization (competition)

---

## FAQ

### Q: Can I modify an approved blueprint?

**A**: No. Approved blueprints are locked. To make changes, reject it, modify, and re-approve. Or create a new blueprint that supersedes the old one.

### Q: What happens if I exceed the budget?

**A**: Execution stops immediately. You'll see a budget exceeded error with current usage stats.

### Q: Can I disable telemetry?

**A**: Yes. Set `codex.telemetry.enabled: false` in VS Code settings. All telemetry is opt-out.

### Q: How do I verify webhook signatures?

**A**: Use HMAC-SHA256 with your webhook secret:

```python
import hmac
import hashlib

def verify_signature(body, signature, secret):
    expected = hmac.new(
        secret.encode(),
        body.encode(),
        hashlib.sha256
    ).hexdigest()
    return hmac.compare_digest(f"sha256={expected}", signature)
```

---

## Troubleshooting

### Blueprint stuck in "pending"

**Solution**: Approve or reject explicitly with `/approve` or `/reject` commands.

### "Approval required" error

**Solution**: Check `codex.research.requireApproval` setting. Network operations require Maintainer role or higher.

### Competition variant merge conflicts

**Solution**: Competition auto-resolves conflicts. If manual intervention needed, check `.codex/worktrees/` for variant branches.

---

## Next Steps

- Read [Execution Modes Guide](./execution-modes.md) for strategy details
- Check [Slash Commands Reference](./slash-commands.md) for full command list
- See [Webhook Setup Guide](./webhooks.md) for integration instructions

---

**Made with ❤️ by zapabob**

