# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is the OpenAI Codex CLI, a terminal-based coding agent that runs interactively and can execute code in sandboxed environments. The repository contains both the TypeScript CLI implementation (`codex-cli/`) and a Rust implementation (`codex-rs/`), plus the Mixture-of-Idiots multi-agent communication framework (`llm_bridge/`).

## Development Commands

This is a **pnpm monorepo**. Commands can be run from the root or specific packages:

### Root-level commands:
```bash
# All commands use pnpm filters to target the TS implementation
pnpm build               # Build the TS CLI
pnpm test                # Run all tests
pnpm lint                # Run linting
pnpm lint:fix            # Auto-fix lint issues
pnpm typecheck           # TypeScript type checking

# Repository maintenance
pnpm format              # Check formatting
pnpm format:fix          # Fix formatting
```

### TypeScript CLI development (run from `codex-cli/`):
```bash
# Development workflow
pnpm dev                 # Watch mode compilation 
pnpm test:watch          # Watch mode testing during development

# Code quality
pnpm typecheck           # TypeScript type checking
pnpm lint                # ESLint checking
pnpm lint:fix            # Auto-fix ESLint issues
pnpm format:fix          # Auto-fix Prettier formatting

# Testing and building
pnpm test                # Run all tests (Vitest)
pnpm build               # Production build
pnpm build:dev           # Development build with source maps

# Run single test file
npx vitest run tests/specific-test.test.ts
```

### Rust CLI development (run from `codex-rs/`):
```bash
just build               # Build Rust CLI
just test                # Run Rust tests
just check               # Check without building
```

## Architecture

### Repository Structure

- **`codex-cli/`**: TypeScript implementation with React/Ink terminal UI
- **`codex-rs/`**: Rust implementation with native TUI
- **`llm_bridge/`**: Mixture-of-Idiots multi-agent communication framework

### TypeScript CLI Core Components (`codex-cli/`)

- **CLI Entry Point**: `src/cli.tsx` - Main entry point with multi-provider support, API key management
- **App Router**: `src/app.tsx` - Root React component managing approval policies and git safety checks
- **Terminal Chat**: `src/components/chat/terminal-chat.tsx` - Main interactive chat interface
- **Agent Loop**: `src/utils/agent/agent-loop.ts` - Core agent execution loop handling tool calls and responses
- **Configuration**: `src/utils/config.ts` - Supports multiple AI providers (OpenAI, Azure, Gemini, etc.)

### Key Operating Modes

1. **Interactive Mode** (default): Full terminal UI with approval prompts
2. **Quiet Mode** (`--quiet`): Non-interactive, prints final output only  
3. **Full Context Mode** (`--full-context`): Loads entire repo context for batch edits
4. **View Mode** (`--view`): Inspect saved rollouts
5. **History Mode** (`--history`): Browse and resume previous sessions

### Approval Policies

The system has three approval modes defined in `src/approvals.ts`:

- **suggest** (default): Prompt for all file writes and commands
- **auto-edit**: Auto-approve file edits, prompt for commands  
- **full-auto**: Auto-approve everything, run commands in sandbox

### Sandboxing

Platform-specific sandboxing implementations in `src/utils/agent/sandbox/`:
- **macOS**: Apple Seatbelt (`sandbox-exec`) with custom policies
- **Linux**: Landlock LSM for filesystem isolation
- Network access blocked by default in full-auto mode

### Multi-Agent Framework (`llm_bridge/`)

The Mixture-of-Idiots system enables human-mediated collaboration between different AI models:

- **Smart Bridge** (`smart_bridge.js`): Message routing and state management
- **Master Control** (`master_control.js`): Human control interface with `/claude` and `/codex` routing
- **Agent Adapters**: Separate interfaces for Claude Code and Codex CLI
- **Launch Script** (`start_mixture.sh`): Automated multi-terminal setup

### Test Framework

- **Vitest** for unit testing with React component support
- **ink-testing-library** for terminal UI testing
- **Rust**: Standard cargo test infrastructure
- Tests follow pattern: `tests/*.test.ts`, `tests/*.test.tsx`

### Configuration System

- **AGENTS.md files**: Hierarchical project documentation (global → repo → local)
- **Multi-provider support**: OpenAI, Azure, Gemini, Ollama, Mistral, etc.
- **Environment-based config**: `.env` files and environment variables
- **YAML/JSON config files**: `~/.codex/config.yaml` or `config.json`

## Key Patterns

- React/Ink components for rich terminal UI
- OpenAI Responses API integration with streaming support
- Shell command parsing and safety assessment via `shell-quote`
- File patching system with path-constrained safety
- Git integration for rollback safety and change tracking
- Provider abstraction for multiple AI services
- Structured logging with debug modes