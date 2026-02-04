---
allowed-tools: Read, Glob, Grep, Write
description: Optimize CLAUDE.md for better LLM project understanding
argument-hint: [target-file-path]
---

## Context

- Target file: $ARGUMENTS (default: CLAUDE.md in project root)
- Workspace Cargo.toml: !`cat cocode-rs/Cargo.toml | head -60`
- Existing CLAUDE.md files: !`find cocode-rs -name "CLAUDE.md" -type f 2>/dev/null`
- Current CLAUDE.md line count: !`wc -l CLAUDE.md 2>/dev/null || echo "not found"`

## Goal

Optimize the CLAUDE.md file to **maximize LLM's ability to understand and work with the project**.

## Optimization Principles

Apply these principles to maximize LLM understanding:

### 1. Conciseness (Context Window Efficiency)
- **Target**: <200 lines
- Remove inline code examples (link to source instead)
- Remove duplicated content (reference AGENTS.md for conventions)

### 2. Progressive Disclosure
- Main file gives overview, links to detailed docs
- LLM can read specialized docs only when needed
- Link to component-specific CLAUDE.md files

### 3. Structured Navigation
- Use tables for crate purposes, not prose
- One-line descriptions per crate
- Group crates by layer

### 4. Architecture Visibility
- Include ASCII art layer diagram
- Show dependency flow: Common → Core → App

### 5. Complete Coverage
- Document ALL workspace crates
- No blind spots for LLM

### 6. No Duplication
- Reference AGENTS.md for code conventions
- Single source of truth per topic

## Task

### Step 1: Analyze Current State

1. Read existing CLAUDE.md (if exists) and note:
   - Current line count (target: <200)
   - Which crates are documented
   - Any duplicated content (should reference AGENTS.md instead)
   - Any inline code examples (should be removed or linked)

2. Read `cocode-rs/Cargo.toml` to get complete crate list from `[workspace] members`

3. Identify gaps:
   - Undocumented crates
   - Missing architecture overview
   - Missing links to specialized CLAUDE.md files

### Step 2: Discover Project Structure

1. Parse workspace members from Cargo.toml
2. Group crates by layer (infer from path):

| Path Prefix | Layer | Purpose |
|-------------|-------|---------|
| `common/` | Common | Foundational types, errors, config |
| `provider-sdks/` | Provider SDKs | LLM provider API clients |
| `core/` | Core | Business logic, agent loop |
| `app/` | App | User-facing (CLI, TUI) |
| `exec/` | Exec | Execution environment |
| `features/` | Features | Optional capabilities |
| `mcp/` | MCP | Model Context Protocol |
| `utils/` | Utils | Utilities |
| Top-level | Standalone | Independent components |

3. For each crate, determine purpose from:
   - Cargo.toml `description` field (if present)
   - Crate name (descriptive)
   - Key dependencies (infer purpose)

4. Find specialized CLAUDE.md files: `cocode-rs/**/CLAUDE.md`

### Step 3: Generate Optimized CLAUDE.md

Create the file with this structure (~150-180 lines):

```
# CLAUDE.md

[One-liner description]

**Read `AGENTS.md` for Rust conventions.** This file covers architecture and crate navigation.

## Commands

Run from `cocode-rs/` directory:

```bash
just fmt          # After Rust changes (auto-approve)
just pre-commit   # REQUIRED before commit
just test         # If changed provider-sdks or core crates
just help         # All commands
```

## Architecture

[ASCII art diagram showing layers and dependencies]

```
┌─────────────────────────────────────────────────────────────────┐
│  App Layer: cli, tui, session                                   │
├─────────────────────────────────────────────────────────────────┤
│  Core Layer: loop → executor → api                              │
│              ↓         ↓                                        │
│           tools ← context ← prompt                              │
│              ↓                                                  │
│        message, system-reminder, subagent                       │
├─────────────────────────────────────────────────────────────────┤
│  Features: skill, hooks, plugin, plan-mode                      │
│  Exec: shell, sandbox, arg0                                     │
│  MCP: mcp-types, rmcp-client                                    │
│  Standalone: retrieval, lsp                                     │
├─────────────────────────────────────────────────────────────────┤
│  Provider SDKs: hyper-sdk → anthropic, openai, google-genai,    │
│                             volcengine-ark, z-ai                │
├─────────────────────────────────────────────────────────────────┤
│  Common: protocol, config, error, otel                          │
│  Utils: [count] utility crates                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Crate Guide ([total] crates)

### Common ([count])
| Crate | Purpose |
|-------|---------|
| ... | ... |

### Provider SDKs ([count])
| Crate | Purpose |
|-------|---------|
| ... | ... |

### Core ([count])
| Crate | Purpose |
|-------|---------|
| ... | ... |

[Continue for all layers...]

## Error Handling

| Layer | Error Type |
|-------|------------|
| common/, core/ | `cocode-error` + snafu |
| provider-sdks/, utils/ | `anyhow::Result` |

See [common/error/README.md](cocode-rs/common/error/README.md) for patterns and StatusCode list.

## Specialized Documentation

| Component | Guide |
|-----------|-------|
| TUI | [app/tui/CLAUDE.md](cocode-rs/app/tui/CLAUDE.md) |
| ... | ... |

## References

- **Code conventions**: `AGENTS.md`
- **Error codes**: `cocode-rs/common/error/README.md`
- **User docs**: `docs/` (getting-started.md, config.md, sandbox.md)
```

### Step 4: Write and Verify

1. Write the optimized CLAUDE.md to the target file
2. Count lines (must be <200)
3. Verify all workspace crates are documented
4. Verify all linked CLAUDE.md files exist

### Step 5: Report Results

Output a summary:

```
## CLAUDE.md Optimization Complete

| Metric | Before | After |
|--------|--------|-------|
| Line count | X | Y |
| Crates documented | X | Y (all) |
| Specialized docs linked | X | Y |

### Changes Applied
- [List of optimizations made]

### Verification
- [ ] Line count < 200
- [ ] All workspace crates documented
- [ ] All linked files exist
- [ ] No duplicated content from AGENTS.md
```

## Important Notes

- Do NOT duplicate content from AGENTS.md (code conventions, error patterns)
- Do NOT include inline code examples (link to source files instead)
- Do NOT use prose descriptions for crates (use tables)
- Do NOT skip any workspace crates (complete coverage)
- Each crate description should be ONE LINE maximum
