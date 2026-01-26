# Cocode Architecture Documentation

This directory contains the architecture design for implementing a Claude Code-compatible agent system in Rust.

## Documents

| Document | Description |
|----------|-------------|
| [goals.md](goals.md) | Project goals and requirements |
| [overview.md](overview.md) | High-level architecture overview |
| [crates.md](crates.md) | Crate organization and dependencies |
| [features.md](features.md) | Key features: slash commands, plan mode, compaction, thinking mode |
| [hooks.md](hooks.md) | Hooks system: 12 event types, handlers, configuration |
| [mcp.md](mcp.md) | MCP (Model Context Protocol) architecture |
| [core-loop.md](core-loop.md) | Agent loop with StreamingToolExecutor |
| [tools.md](tools.md) | Tool system with 5-stage execution pipeline |
| [subagents.md](subagents.md) | Multi-agent system (5 built-in agents) |
| [background.md](background.md) | Background mode architecture (agent vs bash) |
| [execution-modes.md](execution-modes.md) | Advanced execution: iterative, multi-agent coordination |
| [implementation-plan.md](implementation-plan.md) | Phased implementation plan |

## Quick Reference

```
cocode-rs/
├── common/           # Existing (error, protocol, config, otel)
├── core/             # NEW: Core execution engine (7 crates)
│   ├── message/      # Extended message types
│   ├── tools/        # Tool trait, registry, built-in implementations
│   ├── context/      # State management (incl. PermissionMode)
│   ├── prompt/       # Prompt building
│   ├── loop/         # Agent loop driver (incl. compaction)
│   ├── subagent/     # Subagent system (Task tool, context inheritance)
│   └── executor/     # Advanced execution modes (iterative, collab)
├── features/         # NEW: Extensibility features (3 crates)
│   ├── hooks/        # Event-driven hooks
│   ├── skill/        # Slash commands + skills
│   └── plugin/       # Plugin system
├── mcp/              # NEW: MCP layer (3 crates)
│   ├── types/        # MCP protocol types
│   ├── client/       # MCP client + connection manager
│   └── server/       # MCP server (JSON-RPC over stdio)
├── exec/             # NEW: Execution layer (2 crates)
│   ├── sandbox/      # Sandboxing
│   └── shell/        # Command execution
├── app/              # NEW: Application layer (2 crates)
│   ├── session/      # Session management
│   └── cli/          # CLI entry
├── provider-sdks/    # Existing (hyper-sdk, anthropic, openai, etc.)
└── utils/            # Existing utilities
```

**Total: 17 new crates**

## Key Features

| Feature | Location | Description |
|---------|----------|-------------|
| Slash Commands | `features/skill/` | `/commit`, `/review-pr` etc. |
| Plan Mode | `core/tools/` | Read-only exploration workflow |
| Compaction | `core/loop/` | Auto-summarize when context full |
| Hooks | `features/hooks/` | PreToolUse, PostToolUse events |
| Plugins | `features/plugin/` | Third-party extensions |
| Subagents | `core/subagent/` | 5 built-in agents (Bash, general-purpose, Explore, Plan, claude-code-guide) |
| Iterative Execution | `core/executor/` | Multi-run for iterative refinement |
| Multi-Agent Collab | `core/executor/` | spawn_agent, send_input, wait, close_agent |
| Background Tasks | `core/subagent/`, `core/executor/`, `exec/shell/` | Agent + Bash background modes |
| MCP Client | `mcp/client/` | Connect to external MCP servers |
| MCP Server | `mcp/server/` | Expose agent as MCP endpoint |
| Streaming Tools | `core/loop/` | Execute tools during API streaming |

## Key Design Principles

1. **Maximum Claude Code alignment** - Match behavior and patterns
2. **Leverage existing infrastructure** - Reuse hyper-sdk types where possible
3. **Clean crate boundaries** - Small, focused crates with clear responsibilities
4. **Multi-provider by default** - Support all hyper-sdk providers
5. **Multi-agent capable** - Support spawning agents with different configs/tools
6. **Extensibility first** - Hooks, skills, and plugins as core features
7. **MCP integration** - Client for external tools, server for IDE integration
8. **UI-agnostic core** - Event-driven architecture supports CLI, TUI, IDE
