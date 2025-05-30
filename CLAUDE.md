# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is the OpenAI Codex CLI, a terminal-based coding agent that runs interactively and can execute code in sandboxed environments. The CLI is built with React/Ink for the terminal UI and TypeScript throughout.

## Development Commands

Core development commands (run from `codex-cli/` directory):

```bash
# Development workflow
npm run dev              # Watch mode compilation 
npm run test:watch       # Watch mode testing during development

# Code quality
npm run typecheck        # TypeScript type checking
npm run lint            # ESLint checking
npm run lint:fix        # Auto-fix ESLint issues
npm run format:fix      # Auto-fix Prettier formatting

# Testing and building
npm run test            # Run all tests (Vitest)
npm run build           # Production build
npm run build:dev       # Development build with source maps

# Run single test file
npx vitest run tests/specific-test.test.ts
```

## Architecture

### Core Components

- **CLI Entry Point**: `src/cli.tsx` - Main entry point handling flags, API key validation, and mode routing
- **App Router**: `src/app.tsx` - Root React component managing approval policies and git safety checks
- **Terminal Chat**: `src/components/chat/terminal-chat.tsx` - Main interactive chat interface
- **Agent Loop**: `src/utils/agent/agent-loop.ts` - Core agent execution loop handling tool calls and responses

### Key Modes

1. **Interactive Mode** (default): Full terminal UI with approval prompts
2. **Quiet Mode** (`--quiet`): Non-interactive, prints final output only  
3. **Full Context Mode** (`--full-context`): Loads entire repo context for batch edits
4. **View Mode** (`--view`): Inspect saved rollouts

### Approval Policies

The system has three approval modes defined in `src/utils/auto-approval-mode.ts`:

- **suggest** (default): Prompt for all file writes and commands
- **auto-edit**: Auto-approve file edits, prompt for commands  
- **full-auto**: Auto-approve everything, run commands in sandbox

### Sandboxing

Platform-specific sandboxing implementations in `src/utils/agent/sandbox/`:
- **macOS**: Uses Apple Seatbelt (`sandbox-exec`) 
- **Linux**: Docker container with iptables firewall
- Network access blocked by default in full-auto mode

### Test Framework

- **Vitest** for unit testing
- **ink-testing-library** for React component testing
- Tests follow pattern: `tests/*.test.ts` and `tests/*.test.tsx`

## Key Patterns

- React/Ink components for terminal UI
- OpenAI API integration via responses endpoint
- Shell command parsing and safety assessment via `shell-quote`
- File patching system with safety constraints
- Git integration for rollback safety