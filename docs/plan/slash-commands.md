# Plan Slash Commands - Reference

**Version**: 0.57.0

---

## Command Index

| Command | Purpose | Args Required |
|---------|---------|---------------|
| `/Plan on\|off` | Toggle mode | None |
| `/Plan "title"` | Create Plan | Title/goal |
| `/approve <bp-id>` | Approve | Plan ID |
| `/reject <bp-id>` | Reject | Plan ID, reason |
| `/Plan export` | Export files | Plan ID |
| `/mode <mode>` | Set exec mode | Mode name |
| `/deepresearch "query"` | Research | Query string |

---

## `/Plan on|off`

Toggle plan mode on/off.

### Syntax

```bash
/Plan on
/Plan off
```

### Behavior

- **ON**: Enters plan mode, all operations become read-only until approved
- **OFF**: Exits plan mode, returns to normal operation

### Examples

```bash
# Enable plan mode
codex /Plan on

# Disable plan mode
codex /Plan off
```

---

## `/Plan "<title>" [options]`

Create a new Plan.

### Syntax

```bash
/Plan "<title or goal>" [--mode=<mode>] [--budget.tokens=<N>] [--budget.time=<minutes>]
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `title` | string | Required | Plan title or goal |
| `--mode` | enum | orchestrated | Execution mode: single, orchestrated, competition |
| `--budget.tokens` | number | 100000 | Session token cap |
| `--budget.time` | number | 30 | Time cap in minutes |

### Examples

```bash
# Minimal (use defaults)
codex /Plan "Add request logging"

# With mode
codex /Plan "Refactor auth" --mode=orchestrated

# With custom budget
codex /Plan "Optimize query" --mode=competition --budget.tokens=150000 --budget.time=60

# Single mode for simple task
codex /Plan "Fix typo in README" --mode=single
```

### Output

```
✅ Plan created: bp-2025-11-02T12:00:00Z_add-request-logging
📋 Status: drafting
🎯 Mode: orchestrated
```

---

## `/approve <bp-id>`

Approve a Plan for execution.

### Syntax

```bash
/approve <Plan-id>
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `Plan-id` | string | Plan ID to approve |

### Behavior

- Transitions Plan from `pending` → `approved`
- Unlocks execution (file writes, network calls, etc.)
- Requires Maintainer role or higher
- Triggers `Plan.approved` webhook event

### Examples

```bash
# Approve specific Plan
codex /approve bp-2025-11-02T12:00:00Z_add-logging

# Approve current Plan (in VS Code)
codex /approve
```

### Output

```
✅ Plan bp-123 approved by john.doe
🚀 Ready for execution
```

---

## `/reject <bp-id> --reason="..."`

Reject a Plan with reason.

### Syntax

```bash
/reject <Plan-id> --reason="<rejection reason>"
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `Plan-id` | string | Plan ID to reject |
| `--reason` | string | Rejection reason (required) |

### Behavior

- Transitions Plan to `rejected` state
- Logs rejection reason
- Can be reworked (back to `drafting`)
- Triggers `Plan.rejected` webhook event

### Examples

```bash
# Reject with reason
codex /reject bp-123 --reason="Scope too broad, split into smaller tasks"

# Reject current Plan
codex /reject --reason="Security concerns, need audit first"
```

---

## `/Plan export <bp-id> [options]`

Export Plan to markdown and/or JSON.

### Syntax

```bash
/Plan export <Plan-id> [--format=<format>] [--path=<directory>]
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `Plan-id` | string | Required | Plan ID |
| `--format` | enum | both | Export format: md, json, both |
| `--path` | string | docs/Plans | Export directory |

### Examples

```bash
# Export both formats (default)
codex /Plan export bp-123

# Markdown only
codex /Plan export bp-123 --format=md

# Custom path
codex /Plan export bp-123 --path=./my-Plans
```

### Output Files

- **Markdown**: `docs/Plans/2025-11-02_add-logging.md` (human-readable)
- **JSON**: `logs/Plan/bp-123.json` (machine-readable)

---

## `/mode <single|orchestrated|competition>`

Set execution mode for Plans.

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
  "codex.Plan.enabled": true,
  "codex.Plan.mode": "orchestrated",
  "codex.Plan.autoApprove": false,
  "codex.Plan.exportPath": "docs/Plans",
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

For compatibility, `/plan` is aliased to `/Plan` (2 release window).

```bash
# Both work the same
codex /plan "Add feature"
codex /Plan "Add feature"
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
- [Sample Plans](../Plans/samples/)
- [Developer Documentation](./dev/)

---

**Made with ❤️ by zapabob**

