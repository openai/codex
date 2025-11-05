# Codex 1.0.0 - Available Commands

**Version**: 1.0.0  
**Date**: 2025-11-02

---

## Interactive Commands

### Basic Usage

```bash
# Interactive TUI
codex

# Start with prompt
codex "タスク内容"

# With image attachment
codex -i screenshot.png "この画面を実装して"
```

---

## Main Commands

### `codex exec` - Non-interactive execution

```bash
codex exec "explain main.rs"
codex exec "add logging to all API endpoints"
```

**Aliases**: `codex e`

### `codex resume` - Resume previous session

```bash
# Show session picker
codex resume

# Resume most recent
codex resume --last
```

### `codex apply` - Apply latest diff

```bash
# Apply Codex's proposed changes
codex apply
```

**Aliases**: `codex a`

---

## Agent Commands (EXPERIMENTAL)

### `codex delegate` - Delegate to sub-agent

```bash
# Code review
codex delegate code-reviewer --scope ./src

# Security audit
codex delegate sec-audit --scope ./

# Test generation
codex delegate test-gen --scope ./tests
```

**Available Agents**:
- `code-reviewer` - Code review with best practices
- `sec-audit` - Security vulnerability scanning
- `test-gen` - Unit/Integration test generation
- `researcher` - Deep research with citations

### `codex delegate-parallel` - Parallel delegation

```bash
# Run multiple agents simultaneously
codex delegate-parallel code-reviewer,test-gen --scopes ./src,./tests
```

### `codex pair` - Pair programming with supervisor

```bash
codex pair "Implement JWT authentication"
```

Natural language orchestration - supervisor analyzes task and assigns to appropriate agents.

### `codex agent-create` - Create custom agent

```bash
codex agent-create "Find all TODO comments and create summary"
```

### `codex ask` - Ask with @mention

```bash
codex ask "@code-reviewer review this file"
codex ask "@sec-audit check for vulnerabilities"
codex ask "@researcher find best practices for React hooks"
```

**Supported @mentions**:
- `@code-reviewer`
- `@sec-audit`
- `@test-gen`
- `@researcher`

### `codex agent` - Natural language agent invocation

```bash
codex agent "Review with security focus"
codex agent "Generate tests for authentication module"
```

---

## Quick Actions (EXPERIMENTAL)

### `codex review` - Quick review

```bash
codex review ./src/main.rs
codex review ./src/
```

Equivalent to: `codex delegate code-reviewer --scope <path>`

### `codex audit` - Quick audit

```bash
codex audit ./
codex audit ./src/auth.rs
```

Equivalent to: `codex delegate sec-audit --scope <path>`

### `codex test` - Quick test generation

```bash
codex test ./src/auth.rs
codex test ./src/api/
```

Equivalent to: `codex delegate test-gen --scope <path>`

---

## Research Command (EXPERIMENTAL)

### `codex research` - Deep research

```bash
# Quick research
codex research "React Server Components best practices"

# Deep dive
codex research "Rust async error handling" --depth 3

# Comprehensive strategy
codex research "Modern web frameworks" --depth 3 --strategy comprehensive
```

**Options**:
- `--depth <1-5>` - Research depth (default: 2)
- `--strategy <focused|comprehensive|exploratory>` - Research strategy (default: focused)

**Features**:
- Multi-source research (DuckDuckGo, Brave, Google, Bing)
- Cited reports with confidence scores
- Contradiction detection

---

## Utility Commands

### `codex login` / `codex logout`

Manage authentication credentials.

```bash
codex login
codex logout
```

### `codex mcp` - MCP server management

```bash
# Run as MCP server (stdio)
codex mcp-server

# Manage MCP servers (experimental)
codex mcp <subcommands>
```

### `codex sandbox` - Sandbox debugging

```bash
codex sandbox <command>
```

**Aliases**: `codex debug`

### `codex completion` - Shell completion

```bash
# Generate completion script
codex completion bash > ~/.codex-completion.bash
codex completion zsh > ~/.zsh/completions/_codex
codex completion fish > ~/.config/fish/completions/codex.fish
codex completion powershell > $PROFILE
```

### `codex features` - Feature flags

```bash
codex features
```

Inspect available feature flags.

### `codex lock` - Repository locks

```bash
codex lock <subcommands>
```

Manage repository-level locks (experimental).

### `codex webhook` - Webhook notifications

```bash
codex webhook <subcommands>
```

Send notifications to GitHub, Slack, or custom endpoints (experimental).

---

## Cloud Commands (EXPERIMENTAL)

### `codex cloud`

Browse and apply Codex Cloud tasks locally.

```bash
codex cloud
```

---

## Global Options

### Model Selection

```bash
# Specify model
codex -m gpt-5-pro "task"
codex -m claude-4.5-sonnet "task"

# Use local OSS model (Ollama)
codex --oss "task"
```

### Sandbox & Approval

```bash
# Sandbox mode
codex -s read-only "task"
codex -s workspace-write "task"
codex -s danger-full-access "task"  # DANGEROUS!

# Approval policy
codex -a untrusted "task"      # Ask for untrusted commands
codex -a on-failure "task"     # Ask only on failures
codex -a on-request "task"     # Model decides
codex -a never "task"          # Never ask (DANGEROUS!)

# Convenience: low-friction auto execution
codex --full-auto "task"
# Equivalent to: -a on-failure --sandbox workspace-write
```

### Configuration Override

```bash
# Override config.toml values
codex -c model="o3" "task"
codex -c 'sandbox_permissions=["disk-full-read-access"]' "task"

# Enable/disable features
codex --enable web_search "task"
codex --disable telemetry "task"
```

### Working Directory

```bash
# Set working directory
codex -C /path/to/project "task"

# Add writable directories
codex --add-dir /tmp/output "task"
```

### Web Search

```bash
# Enable web search
codex --search "Find latest React best practices and implement"
```

### Profile

```bash
# Use configuration profile
codex -p production "deploy"
codex -p development "test locally"
```

---

## Common Workflows

### 1. Code Review

```bash
# Quick review
codex review ./src

# Or detailed
codex delegate code-reviewer --scope ./src
```

### 2. Security Audit

```bash
codex audit ./
```

### 3. Test Generation

```bash
codex test ./src/auth.rs
```

### 4. Feature Implementation

```bash
# Simple task
codex "Add logging middleware"

# Complex task with orchestration
codex pair "Implement JWT authentication with tests and security review"
```

### 5. Research + Implementation

```bash
# Step 1: Research
codex research "JWT authentication best practices" --depth 3

# Step 2: Implement
codex pair "Implement JWT auth based on research findings"
```

### 6. Parallel Code Review + Test Generation

```bash
codex delegate-parallel code-reviewer,test-gen --scopes ./src,./tests
```

---

## Tips & Best Practices

### 🔒 Security First

```bash
# ✅ GOOD: Safe execution
codex -s workspace-write -a on-request "task"

# ❌ BAD: Dangerous auto-approval
codex -a never -s danger-full-access "task"
```

### 🚀 Performance

```bash
# Use gpt-5-mini for quick tasks
codex -m gpt-5-mini "add docstring"

# Use gpt-5-pro for complex tasks
codex -m gpt-5-pro "refactor architecture"
```

### 🎯 Task Complexity

| Complexity | Command | Example |
|------------|---------|---------|
| Simple | `codex "task"` | Typo fix, docstring |
| Medium | `codex pair "task"` | Multi-file feature |
| Complex | `codex pair + research` | Full-stack refactor |

---

## Future Features (Not Yet Available)

The following features are **planned but not yet implemented** in v1.0.0:

### Blueprint Mode (Planned for v0.57.0+)

- `/blueprint on|off` - Toggle blueprint mode
- `/blueprint "title"` - Create blueprint
- `/approve <bp-id>` - Approve blueprint
- `/reject <bp-id>` - Reject blueprint
- `/mode <single|orchestrated|competition>` - Set execution mode

**Status**: Documentation exists, but implementation pending.

---

## Getting Help

```bash
# Main help
codex --help

# Subcommand help
codex delegate --help
codex research --help
```

---

**Made with ❤️ by zapabob**


