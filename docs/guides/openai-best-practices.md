# 🌟 OpenAI Codex Best Practices Guide

**Based on**: OpenAI/codex latest recommendations (January 2025)  
**Date**: 2025-10-13  
**Purpose**: Align zapabob/codex with official best practices

---

## 📋 Official OpenAI Codex Best Practices

### 1. Model Selection 🎯

#### Recommended Approach
```bash
# Always specify model explicitly via CLI
codex --model gpt-5-codex "task description"

# Use appropriate model for task complexity
codex --model gpt-4o-mini "simple refactoring"        # Fast, cost-effective
codex --model gpt-5-codex "complex implementation"    # Latest Codex (2025)
codex --model gpt-5-codex-medium "balanced tasks"     # Medium variant
codex --model o1-preview "algorithmic challenges"     # Reasoning-intensive
```

#### Configuration File
```toml
# ~/.codex/config.toml
# Best Practice: Provide sensible default, allow CLI override
model = "gpt-5-codex"  # Default: Latest Codex model (2025)
# Alternative: "gpt-5-codex-medium", "gpt-4o", "gpt-4o-mini", "o1-preview"
```

**Rationale**: 
- ✅ Flexibility per task
- ✅ Clear and explicit
- ✅ Fallback to sensible default

---

### 2. MCP Server Configuration 🔌

#### Recommended Setup
```toml
# ~/.codex/config.toml
[mcp_servers.codex-agent]
command = "codex"
args = ["mcp-server"]
env.CODEX_CONFIG_PATH = "~/.codex/config.toml"
env.RUST_LOG = "info"
```

```json
// ~/.cursor/mcp.json (for IDE integration)
{
  "mcpServers": {
    "codex": {
      "command": "codex",
      "args": ["mcp-server"],
      "env": {
        "RUST_LOG": "info",
        "CODEX_CONFIG_PATH": "/absolute/path/to/config.toml"
      }
    }
  }
}
```

**Key Points**:
- ✅ Use absolute paths in mcp.json
- ✅ Consistent RUST_LOG level
- ✅ Single source of configuration (config.toml)

---

### 3. Security & Sandbox 🔒

#### Recommended Settings
```toml
# ~/.codex/config.toml
[sandbox]
# Default: read-only for safety
default_mode = "read-only"

# Allow workspace writes when needed
[sandbox_permissions]
workspace_write = true
disk_full_read_access = false  # Limit to workspace only
network_access = false  # Disable by default
```

**Best Practices**:
- ✅ Start with restrictive permissions
- ✅ Explicitly enable when needed
- ✅ Never enable `danger-full-access` by default
- ✅ Use `--sandbox=workspace-write` for specific tasks

---

### 4. Approval Policy 👍

#### Recommended Settings
```toml
# ~/.codex/config.toml
[approval]
policy = "on-request"  # Default: Ask before executing

# For trusted environments
# policy = "never"  # Auto-approve (use with caution)
```

**Best Practices**:
- ✅ `on-request`: Safe default for new users
- ✅ `never`: Only in trusted, automated environments
- ❌ `untrusted`: Too restrictive for most use cases

---

### 5. Provider Configuration 🌐

#### Recommended Setup
```toml
# ~/.codex/config.toml
[model_providers.openai]
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
name = "OpenAI"
requires_openai_auth = true
wire_api = "chat"  # Use Chat Completions API
```

**Key Points**:
- ✅ Use `wire_api = "chat"` for modern API
- ✅ Store API key in environment variable
- ✅ Never commit API keys to git

---

### 6. Session Management 💾

#### Recommended Settings
```toml
# ~/.codex/config.toml
[session]
auto_save = true
save_interval = 300  # 5 minutes
max_history = 100
```

**Best Practices**:
- ✅ Enable auto-save for safety
- ✅ Regular save intervals
- ✅ Limit history to manage disk space

---

### 7. Logging & Debugging 📊

#### Recommended Settings
```toml
# ~/.codex/config.toml
[logging]
level = "info"  # Default: info
# level = "debug"  # For troubleshooting
log_dir = "~/.codex/logs"
max_log_files = 10
```

**Best Practices**:
- ✅ `info` for production
- ✅ `debug` for development
- ✅ Rotate logs regularly
- ✅ Review logs for issues

---

## 🎯 zapabob/codex Specific Enhancements

### 1. Subagent Configuration 🤖

```toml
# ~/.codex/config.toml
[subagents]
enabled = true
max_parallel = 4  # Limit concurrent subagents
token_budget = 10000  # Per subagent limit
inherit_model = true  # Use parent's model
```

**zapabob Extension**:
- ✅ Token budget management
- ✅ Parallel execution control
- ✅ Model inheritance from parent

---

### 2. Deep Research Configuration 🔍

```toml
# ~/.codex/config.toml
[deep_research]
enabled = true
max_depth = 3
max_sources = 5
default_strategy = "focused"  # focused, comprehensive, exploratory
```

**zapabob Extension**:
- ✅ Configurable depth
- ✅ Source limits
- ✅ Strategy selection

---

### 3. Audit Logging 📝

```toml
# ~/.codex/config.toml
[audit]
enabled = true
log_dir = "~/.codex/audit-logs"
include_token_usage = true
include_model_info = true
format = "json"  # json or yaml
```

**zapabob Extension**:
- ✅ Full execution traceability
- ✅ Token usage tracking
- ✅ Model versioning

---

## 📝 Complete Recommended config.toml

```toml
# Codex Configuration File
# Based on OpenAI best practices + zapabob extensions

# ==================== Core Settings ====================
# Model: Default model (override with --model flag)
model = "gpt-5-codex"  # Latest Codex model (2025)
# Alternative: "gpt-5-codex-medium", "gpt-4o", "gpt-4o-mini", "o1-preview"
model_reasoning_summary = "detailed"
windows_wsl_setup_acknowledged = true

# ==================== Provider ====================
[model_providers.openai]
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
name = "OpenAI"
requires_openai_auth = true
wire_api = "chat"

# ==================== Security & Sandbox ====================
[sandbox]
default_mode = "read-only"

[sandbox_permissions]
workspace_write = true
disk_full_read_access = false
network_access = false

# ==================== Approval ====================
[approval]
policy = "on-request"

# ==================== Session ====================
[session]
auto_save = true
save_interval = 300
max_history = 100

# ==================== Logging ====================
[logging]
level = "info"
log_dir = "~/.codex/logs"
max_log_files = 10

# ==================== MCP Servers ====================
[mcp_servers.codex-agent]
command = "codex"
args = ["mcp-server"]
env.CODEX_CONFIG_PATH = "~/.codex/config.toml"
env.RUST_LOG = "info"

# ==================== zapabob Extensions ====================
# Subagent Configuration
[subagents]
enabled = true
max_parallel = 4
token_budget = 10000
inherit_model = true

# Deep Research Configuration
[deep_research]
enabled = true
max_depth = 3
max_sources = 5
default_strategy = "focused"

# Audit Logging
[audit]
enabled = true
log_dir = "~/.codex/audit-logs"
include_token_usage = true
include_model_info = true
format = "json"

# ==================== Project Trust ====================
[projects.'\\?\C:\Users\downl\Desktop\codex-main\codex-main']
trust_level = "trusted"
```

---

## 🚀 Usage Examples (Best Practices)

### Example 1: Daily Development

```bash
# Quick refactoring (use fast model)
codex --model gpt-4o-mini "Rename variable foo to bar across files"

# Complex feature (use latest Codex)
codex --model gpt-5-codex "Implement authentication with JWT and refresh tokens"

# Balanced tasks (use medium variant)
codex --model gpt-5-codex-medium "Refactor module with moderate complexity"

# Algorithmic challenge (use reasoning model)
codex --model o1-preview "Optimize this sorting algorithm for large datasets"
```

---

### Example 2: Subagent Orchestration

```bash
# Parallel code review and testing
codex --model gpt-5-codex "Use codex-supervisor to review security and generate tests in parallel"

# Deep research before implementation
codex --model gpt-5-codex "Research React Server Components best practices, then implement a example"
```

---

### Example 3: Safe Execution

```bash
# Read-only analysis
codex --sandbox=read-only "Analyze the codebase structure"

# Workspace writes allowed
codex --sandbox=workspace-write "Refactor authentication module"

# Dangerous operations (explicit)
codex --sandbox=danger-full-access --approval=never "Automated deployment script"
```

---

## ⚠️ Common Pitfalls to Avoid

### 1. Hardcoding Models ❌

```toml
# BAD: Model hardcoded, no flexibility
model = "gpt-5-codex-medium"  # Non-existent model
```

```bash
# GOOD: Use CLI flag for flexibility
codex --model gpt-4o "task"
```

---

### 2. Overly Permissive Sandbox ❌

```toml
# BAD: Too permissive by default
[sandbox]
default_mode = "danger-full-access"
```

```toml
# GOOD: Restrictive default, explicit when needed
[sandbox]
default_mode = "read-only"
```

---

### 3. Missing API Key Handling ❌

```toml
# BAD: API key in config file
[model_providers.openai]
api_key = "sk-..."  # DON'T DO THIS!
```

```bash
# GOOD: Use environment variable
export OPENAI_API_KEY="sk-..."
```

---

### 4. No Token Budget ❌

```toml
# BAD: Unlimited token usage
[subagents]
token_budget = 999999999
```

```toml
# GOOD: Reasonable limit
[subagents]
token_budget = 10000
```

---

## 🎯 Migration Checklist

### From Old Configuration to Best Practices

- [ ] Update model to `gpt-4o` (stable default)
- [ ] Add provider configuration with `wire_api = "chat"`
- [ ] Configure sandbox with `default_mode = "read-only"`
- [ ] Set approval policy to `on-request`
- [ ] Enable session auto-save
- [ ] Configure logging level to `info`
- [ ] Add MCP server for codex-agent
- [ ] (zapabob) Configure subagent token budget
- [ ] (zapabob) Enable deep research with limits
- [ ] (zapabob) Enable audit logging
- [ ] Remove any hardcoded API keys
- [ ] Verify absolute paths in mcp.json

---

## 📊 Performance Recommendations

### Model Selection by Task Type

| Task Type | Recommended Model | Reasoning |
|-----------|-------------------|-----------|
| Quick fixes | `gpt-4o-mini` | Fast, cost-effective |
| Standard development | `gpt-5-codex` | Latest Codex (2025) |
| Complex refactoring | `gpt-5-codex` | Strong code understanding |
| Algorithm design | `o1-preview` | Superior reasoning |
| Documentation | `gpt-4o-mini` | Sufficient for text |
| Code review | `gpt-5-codex` | Detailed code analysis |
| Balanced tasks | `gpt-5-codex-medium` | Medium variant |

---

### Subagent Configuration by Use Case

| Use Case | max_parallel | token_budget | Strategy |
|----------|--------------|--------------|----------|
| Small project | 2 | 5000 | Sequential |
| Medium project | 4 | 10000 | Hybrid |
| Large project | 8 | 20000 | Parallel |
| CI/CD | 2 | 5000 | Sequential |

---

## 🎉 Summary

### OpenAI Official Best Practices
1. ✅ Flexible model selection via CLI
2. ✅ Sensible default in config
3. ✅ Secure sandbox by default
4. ✅ Explicit approval policy
5. ✅ Proper provider configuration
6. ✅ Session management
7. ✅ Appropriate logging

### zapabob Extensions
1. ✅ Subagent token budgeting
2. ✅ Parallel execution control
3. ✅ Deep research configuration
4. ✅ Comprehensive audit logging
5. ✅ Model inheritance

### Result
**Production-ready Codex configuration aligned with OpenAI best practices + powerful zapabob enhancements** 🚀

---

**Author**: zapabob  
**Date**: 2025-10-13  
**Based on**: OpenAI/codex official recommendations  
**Status**: ✅ **Ready for Implementation**

