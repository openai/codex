# Codex CLI Project Context

## Project Overview

**Codex CLI** is an advanced local coding agent developed by OpenAI. It is designed to assist developers directly in their terminal or integrated development environments (IDEs). The project is a monorepo primarily built with **Rust**, featuring a modular architecture that separates core logic, CLI interface, TUI components, and protocol definitions.

### Key Technologies
*   **Language:** Rust (Workspace with multiple crates)
*   **Package Manager:** Cargo (Rust), pnpm (Node.js/TypeScript assets)
*   **Task Runner:** `just`
*   **Protocols:** Model Context Protocol (MCP) support

## Operational Strategy (Agents vs. Tools)

We follow this priority order for complex analysis and investigation tasks:

1.  **Priority 1: Internal Subagents (`delegate_to_agent`)**
    *   **Tool:** `delegate_to_agent` (specifically `codebase_investigator`).
    *   **Use Case:** The primary method for architectural mapping, dependency tracking, and deep codebase understanding. Use this for the majority of investigation tasks.

2.  **Priority 2: Codex CLI Tool (`codex exec`)**
    *   **Tool:** `run_shell_command("codex exec ...")`
    *   **Use Case:** A fallback or specialist tool. Use ONLY when:
        *   The task requires extremely niche knowledge that the internal agent failed to provide.
        *   Deep "expert-level" backend logic analysis is required (using `--model gpt-5-codex`).
        *   The user explicitly requests using Codex for the analysis.

## Using Codex CLI (The Tool)

If `codex` is installed in the path, use these patterns to leverage its capabilities as a Secondary/Expert tool.

### Non-Interactive / Quick Analysis
In environments without a full TTY (like this agent context), use `exec` to bypass the interactive UI.

```bash
codex exec "Explain the logic in this directory"
```

### Model Selection Strategy
Select the appropriate model based on task complexity:

*   **Fast / Specific Functions:** Use `gpt-5.1-codex-mini` for targeted tasks, diagram summarization, or simple refactors.
    ```bash
    codex exec --model gpt-5.1-codex-mini "Summarize these diagrams"
    ```
*   **Expert / Deep Backend:** Use `gpt-5-codex` for complex architectural analysis, deep logic debugging, or "expert level" backend tasks.
    ```bash
    codex exec --model gpt-5-codex "Analyze the concurrency logic in subagents.rs"
    ```

### Key Flags
*   `--cd <DIR>`: Run codex in a specific directory without `cd`-ing first.
*   `--search`: Enable web search capabilities.
*   `--sandbox <MODE>`: Set sandbox policy (`read-only`, `workspace-write`, `danger-full-access`).

## Building and Developing (The Repo)

Instructions for modifying the Codex CLI codebase itself.

### Rust Workspace (`codex-rs/`)

*   **Run the CLI (Dev Build):**
    ```bash
    just codex [args]
    # OR
    cargo run --bin codex -- [args]
    ```

*   **Run the TUI:**
    ```bash
    just tui
    # OR
    cargo run --bin codex -- tui
    ```

*   **Run Tests:**
    ```bash
    just test
    # Uses cargo-nextest
    ```

*   **Format & Fix:**
    ```bash
    just fmt
    just fix
    ```

### Node.js / TypeScript

*   **Format non-Rust files:**
    ```bash
    pnpm run format
    ```

## Development Conventions

*   **Agent Guidelines:** Strictly follow the instructions in `AGENTS.md`. This file contains critical rules for AI agents modifying the codebase, including sandbox constraints and testing procedures.
*   **Crate Naming:** Rust crates are prefixed with `codex-` (e.g., `codex-core`, `codex-cli`).
*   **Formatting:**
    *   Rust: `cargo fmt` with specific config (handled by `just fmt`).
    *   Other: `prettier` (handled by `pnpm run format`).
*   **Testing:**
    *   Prefer `cargo nextest` for running tests.
    *   Integration tests are widely used; ensure environment consistency (see `AGENTS.md` regarding sandboxing).
*   **Subagents:** The project implements a "Subagents" feature (v2), allowing for parallel and sequential execution of specialized agents (Explore, Plan, General). Use `codex-rs/core/src/subagents.rs` for core logic reference.
