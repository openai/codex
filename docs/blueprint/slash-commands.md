# Blueprint Slash Commands - Reference

**Version**: 0.57.0

---

## Command Index

| Command | Purpose | Args Required |
|---------|---------|---------------|
| `/blueprint on\|off` | Toggle mode | None |
| `/blueprint "title"` | Create blueprint | Title/goal |
| `/approve <bp-id>` | Approve | Blueprint ID |
| `/reject <bp-id>` | Reject | Blueprint ID, reason |
| `/blueprint export` | Export files | Blueprint ID |
| `/mode <mode>` | Set exec mode | Mode name |
| `/deepresearch "query"` | Research | Query string |

---

## `/blueprint on|off`

Toggle blueprint mode on/off.

### Syntax

```bash
/blueprint on
/blueprint off
```

### Behavior

- **ON**: Enters blueprint mode, all operations become read-only until approved
- **OFF**: Exits blueprint mode, returns to normal operation

### Examples

```bash
# Enable blueprint mode
codex /blueprint on

# Disable blueprint mode
codex /blueprint off
```

---

## `/blueprint "<title>" [options]`

Create a new blueprint.

### Syntax

```bash
/blueprint "<title or goal>" [--mode=<mode>] [--budget.tokens=<N>] [--budget.time=<minutes>]
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `title` | string | Required | Blueprint title or goal |
| `--mode` | enum | orchestrated | Execution mode: single, orchestrated, competition |
| `--budget.tokens` | number | 100000 | Session token cap |
| `--budget.time` | number | 30 | Time cap in minutes |

### Examples

```bash
# Minimal (use defaults)
codex /blueprint "Add request logging"

# With mode
codex /blueprint "Refactor auth" --mode=orchestrated

# With custom budget
codex /blueprint "Optimize query" --mode=competition --budget.tokens=150000 --budget.time=60

# Single mode for simple task
codex /blueprint "Fix typo in README" --mode=single
```

### Output

```
✅ Blueprint created: bp-2025-11-02T12:00:00Z_add-request-logging
📋 Status: drafting
🎯 Mode: orchestrated
```

---

## `/approve <bp-id>`

Approve a blueprint for execution.

### Syntax

```bash
/approve <blueprint-id>
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `blueprint-id` | string | Blueprint ID to approve |

### Behavior

- Transitions blueprint from `pending` → `approved`
- Unlocks execution (file writes, network calls, etc.)
- Requires Maintainer role or higher
- Triggers `blueprint.approved` webhook event

### Examples

```bash
# Approve specific blueprint
codex /approve bp-2025-11-02T12:00:00Z_add-logging

# Approve current blueprint (in VS Code)
codex /approve
```

### Output

```
✅ Blueprint bp-123 approved by john.doe
🚀 Ready for execution
```

---

## `/reject <bp-id> --reason="..."`

Reject a blueprint with reason.

### Syntax

```bash
/reject <blueprint-id> --reason="<rejection reason>"
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `blueprint-id` | string | Blueprint ID to reject |
| `--reason` | string | Rejection reason (required) |

### Behavior

- Transitions blueprint to `rejected` state
- Logs rejection reason
- Can be reworked (back to `drafting`)
- Triggers `blueprint.rejected` webhook event

### Examples

```bash
# Reject with reason
codex /reject bp-123 --reason="Scope too broad, split into smaller tasks"

# Reject current blueprint
codex /reject --reason="Security concerns, need audit first"
```

---

## `/blueprint export <bp-id> [options]`

Export blueprint to markdown and/or JSON.

### Syntax

```bash
/blueprint export <blueprint-id> [--format=<format>] [--path=<directory>]
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `blueprint-id` | string | Required | Blueprint ID |
| `--format` | enum | both | Export format: md, json, both |
| `--path` | string | docs/blueprints | Export directory |

### Examples

```bash
# Export both formats (default)
codex /blueprint export bp-123

# Markdown only
codex /blueprint export bp-123 --format=md

# Custom path
codex /blueprint export bp-123 --path=./my-blueprints
```

### Output Files

- **Markdown**: `docs/blueprints/2025-11-02_add-logging.md` (human-readable)
- **JSON**: `logs/blueprint/bp-123.json` (machine-readable)

---

## `/mode <single|orchestrated|competition>`

Set execution mode for blueprints.

### Syntax

```bash
/mode <mode>
```

### Parameters

| Parameter | Values | Default |
|-----------|--------|---------|
| `mode` | single, orchestrated, competition | orchestrated |

### Examples

```bash
# Set to orchestrated (default)
codex /mode orchestrated

# Enable competition mode
codex /mode competition

# Simple single-agent mode
codex /mode single
```

---

## `/deepresearch "<query>" [options]`

Conduct deep research with approval dialog.

### Syntax

```bash
/deepresearch "<query>" [--depth=<1-3>] [--policy=<strategy>]
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `query` | string | Required | Research query |
| `--depth` | number (1-3) | 2 | Search depth |
| `--policy` | enum | focused | Strategy: focused, comprehensive, exploratory |

### Strategies

- **focused**: 3-5 sources, specific question
- **comprehensive**: 5-10 sources, deep analysis
- **exploratory**: 10-15 sources, broad survey

### Examples

```bash
# Quick research (depth 1)
codex /deepresearch "FastAPI JWT best practices" --depth=1

# Deep research (depth 3)
codex /deepresearch "Rust async patterns" --depth=3 --policy=comprehensive

# Broad survey
codex /deepresearch "Modern web frameworks" --policy=exploratory
```

### Approval Dialog

```
Research Request:

Query: FastAPI JWT best practices
Depth: 2
Domains: duckduckgo.com, github.com, docs.rs
Token Budget: ~25,000 tokens
Time Budget: ~3 minutes
Data Retention: 30 days, then auto-deleted

[Approve] [Reject]
```

---

## Configuration

### VS Code Settings

```json
{
  "codex.blueprint.enabled": true,
  "codex.blueprint.mode": "orchestrated",
  "codex.blueprint.autoApprove": false,
  "codex.blueprint.exportPath": "docs/blueprints",
  "codex.competition.numVariants": 2,
  "codex.competition.weights.tests": 0.5,
  "codex.competition.weights.performance": 0.3,
  "codex.competition.weights.simplicity": 0.2,
  "codex.research.requireApproval": true,
  "codex.webhooks.enabled": false,
  "codex.telemetry.enabled": true
}
```

---

## Compatibility

### `/plan` Alias

For compatibility, `/plan` is aliased to `/blueprint` (2 release window).

```bash
# Both work the same
codex /plan "Add feature"
codex /blueprint "Add feature"
```

**Disable alias**:
```json
{
  "codex.compat.planAlias": false
}
```

---

## See Also

- [Execution Modes Guide](./execution-modes.md)
- [Webhook Setup](./webhooks.md)
- [Sample Blueprints](../blueprints/samples/)
- [Developer Documentation](./dev/)

---

**Made with ❤️ by zapabob**

