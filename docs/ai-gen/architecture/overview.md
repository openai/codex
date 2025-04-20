# Codex Architecture Overview

## High-Level Architecture

Codex is a terminal-based AI coding assistant that follows a client-server architecture with a stateful agent loop at its core. The key architectural components are:

1. **CLI Interface** - Terminal-based React application built with Ink
2. **Agent Loop** - Core orchestration logic for the AI-driven workflow
3. **Tool System** - Extensible framework for executing commands and code edits
4. **Security Sandbox** - Platform-specific command execution sandboxing
5. **Context Management** - Sophisticated handling of input context and model outputs

```
┌─────────────┐          ┌─────────────────┐          ┌───────────────┐
│ CLI         │          │ Agent Loop      │          │ OpenAI API    │
│ (Ink/React) │ ◄──────► │ (Orchestrator)  │ ◄──────► │ (o4-mini/o3)  │
└─────────────┘          └─────────────────┘          └───────────────┘
                                  ▲
                                  │
                                  ▼
                          ┌───────────────┐
                          │ Tool System   │
                          │ (File/Shell)  │
                          └───────────────┘
                                  ▲
                                  │
                                  ▼
                          ┌───────────────┐
                          │ Sandbox       │
                          │ (Security)    │
                          └───────────────┘
```

## Key Components

### CLI Interface (src/components/)

The user interface is built with [Ink](https://github.com/vadimdemedes/ink), a React-based framework for building CLI applications. The interface provides a terminal-based chat experience, handling user input, command history, message display, and tool output rendering.

### Agent Loop (src/utils/agent/agent-loop.ts)

The Agent Loop is the central orchestration component. It:

1. Manages the conversation state and context
2. Handles communication with the OpenAI API
3. Processes model outputs including function calls
4. Manages command approval and execution
5. Handles error states and retries for API calls
6. Provides cancellation and interruption capabilities

### Tool System (src/utils/agent/)

The tool system allows the LLM to interact with the local environment through:

1. `shell` - Command execution tool
2. `apply_patch` - Precise file editing capability

Tools are implemented as "function calls" in the OpenAI API, providing a structured interface for the model to invoke actions, with argument validation and sandboxed execution.

### Context Management (src/utils/singlepass/)

Sophisticated context building for model inputs, including:

1. Task context preparation with XML-formatted file content
2. Context size management and optimization
3. Directory structure representation
4. Path manipulation and filtering

### Security (src/utils/agent/sandbox/)

Security mechanisms include:

1. Command approval system with different policies
2. Platform-specific sandboxing (macOS Seatbelt)
3. Writable paths restrictions
4. Review process for potentially dangerous operations

## Data Flow

1. User inputs a natural language prompt
2. CLI processes and sends to Agent Loop
3. Agent Loop packages context and sends to OpenAI API
4. Model responds with text or function calls
5. Function calls are executed via the Tool System
6. Tool outputs are sent back to the model
7. Process continues until task is complete

## Design Principles

1. **Terminal-First**: Built for developer workflow in the terminal
2. **Tool-Based**: Structured around tool-calling for execution
3. **Security-Conscious**: Approval-based with sandboxed execution
4. **Context-Aware**: Sophisticated context handling for model performance
5. **Error Resilient**: Robust error handling and retry mechanisms