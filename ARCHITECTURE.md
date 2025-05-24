# Codex Architecture Overview

This document outlines the architecture of Codex, a system designed for AI-driven code generation and manipulation. It comprises two main components: a command-line interface (`codex-cli`) and a backend engine (`codex-rs`).

## Components

### `codex-cli` (TypeScript/React/Ink)

The `codex-cli` is the user-facing component of the system. It is responsible for:

*   **User Interaction:** Providing a command-line interface for users to interact with Codex. This includes inputting prompts, viewing progress, and managing sessions.
*   **UI Rendering:** Using React and Ink to render a rich, interactive terminal interface. Key files include `cli.tsx` (main entry point), `App.tsx` (core application logic), and `AgentLoop` (manages the interaction loop with the backend).
*   **Communication:** Sending user requests to the `codex-rs` backend and receiving updates to display to the user.

### `codex-rs` (Rust)

The `codex-rs` is the backend engine that powers the core functionality of Codex. It is responsible for:

*   **AI Model Interaction:** Communicating with the underlying AI model to generate code, suggest changes, and perform other language-related tasks.
*   **Task Management:** Managing `AgentTask` instances, which represent individual units of work being processed by the AI.
*   **Code Manipulation:** Applying changes to codebases, including applying patches and executing commands in a sandboxed environment.
*   **Extensibility:** Providing a modular architecture that allows for the addition of new tools and capabilities.

## Communication Protocol

Communication between `codex-cli` and `codex-rs` is handled through two main queues, defined in `codex-rs/core/src/protocol.rs`:

*   **`Submission` Queue (CLI to Rust):** The CLI sends `Submission` messages to `codex-rs`. These messages represent user requests, such as a new prompt or a command to execute.
*   **`Event` Queue (Rust to CLI):** `codex-rs` sends `Event` messages back to the CLI. These messages represent updates on the status of tasks, results from the AI model, or requests for user input (e.g., approval for a patch).

## Key Modules/Crates in `codex-rs`

The `codex-rs` backend is organized into several key modules and crates:

*   **`core`:** This is the central crate containing the main logic of Codex.
    *   **`Codex` struct:** The primary entry point and orchestrator for backend operations.
    *   **`Session`:** Manages the state of a user's interaction with Codex, including the current context and history.
    *   **`AgentTask`:** Represents a specific task being processed by the AI, such as generating code for a given prompt.
    *   **Model Interaction:** Handles communication with the AI model.
    *   **Function Call Handling:** Manages the execution of function calls requested by the AI model (e.g., running a shell command or applying a patch).

*   **`apply-patch`:** This crate is responsible for applying unified diffs (patches) to files. It ensures that changes are applied correctly and handles potential conflicts.

*   **`mcp-*` (Multi-Capability Protocol):** This set of crates defines the Multi-Capability Protocol, which allows for extending Codex with new tools and agents. It provides a standardized way for different components to communicate and interact.

*   **`exec` & `safety`:** These crates are responsible for sandboxed command execution.
    *   `exec`: Provides the mechanisms for running external commands.
    *   `safety`: Implements security measures to ensure that commands are executed in a controlled and safe environment, preventing unintended side effects.

## Typical Data Flow

A typical interaction with Codex follows this data flow:

1.  **User Input:** The user provides input through the `codex-cli` (e.g., a prompt to generate a new function).
2.  **CLI Processing:** The CLI (`cli.tsx`, `App.tsx`, `AgentLoop`) processes the input and prepares a `Submission` message.
3.  **Submission to Backend:** The `Submission` is sent to `codex-rs`.
4.  **Backend Processing:**
    *   The `submission_loop` in `codex-rs` receives the `Submission`.
    *   An `AgentTask` is created to handle the request.
    *   The `AgentTask` interacts with the AI Model.
5.  **AI Model Interaction:** The AI model processes the request and may generate code or suggest actions.
6.  **Potential Function Call:** If the AI model requests a function call (e.g., to run a shell command via the `exec` crate or apply a patch via `apply-patch`), `codex-rs` handles its execution.
7.  **Event Generation:** `codex-rs` generates `Event` messages to update the CLI on the progress and results.
8.  **CLI UI Update:** The CLI receives the `Event` messages and updates the user interface accordingly, displaying generated code, asking for approval, or showing command output.

## Modularity and Control

Codex is designed with modularity and control in mind:

*   **CLI/Backend Split:** The separation of the `codex-cli` and `codex-rs` allows for independent development and deployment. The CLI can be updated without affecting the backend, and vice-versa.
*   **Rust Crates:** The use of distinct Rust crates in `codex-rs` promotes code organization, reusability, and maintainability. Each crate has a specific responsibility, making the system easier to understand and extend.
*   **MCP (Multi-Capability Protocol):** MCP enables the addition of new tools and agents without requiring significant changes to the core system. This promotes extensibility and allows Codex to adapt to new use cases.
*   **`approvalMode`:** This feature (if implemented as suggested by its name) would provide users with control over potentially destructive actions, such as applying patches or running commands, by requiring explicit approval.
*   **Sandboxing:** The `exec` and `safety` crates ensure that any commands executed by Codex are run in a sandboxed environment, limiting potential security risks and preventing unintended system modifications.
