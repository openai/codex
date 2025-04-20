# Tools Implementation in Codex

## Overview

Codex implements a tool-based architecture that allows the AI model to interact with the local environment in a controlled manner. The tool system is centered around "function calling" capabilities of the OpenAI API, providing structured interfaces for executing commands and modifying files.

## Available Tools

Codex currently implements two primary tools:

1. **Shell Command Tool**
   - Function name: `shell`
   - Allows the model to execute shell commands in the local environment
   - Supports timeout configuration and working directory specification
   - Captures standard output, standard error, and exit code

2. **Apply Patch Tool**
   - Function name: `apply_patch` (used through the shell tool)
   - Enables precise file editing through unified diff patches
   - Handles file creation, modification, and deletion
   - Maintains record of changes for approval

## Tool Implementation

### Shell Command Tool

```json
{
  "type": "function",
  "name": "shell",
  "description": "Runs a shell command, and returns its output.",
  "strict": false,
  "parameters": {
    "type": "object",
    "properties": {
      "command": { 
        "type": "array", 
        "items": { "type": "string" } 
      },
      "workdir": {
        "type": "string",
        "description": "The working directory for the command."
      },
      "timeout": {
        "type": "number",
        "description": "The maximum time to wait for the command to complete in milliseconds."
      }
    },
    "required": ["command"],
    "additionalProperties": false
  }
}
```

The shell tool is implemented in `src/utils/agent/exec.ts` and invoked through the `handleExecCommand` function in `src/utils/agent/handle-exec-command.ts`.

### Tool Execution Flow

1. Model generates a function call with tool name and arguments
2. Agent Loop processes the function call
3. Arguments are validated and normalized
4. Command is evaluated against approval policy
5. User confirmation is requested if needed
6. Command is executed in appropriate sandbox
7. Output is captured and returned to the model
8. Model continues processing with tool output

## Approval System

The tool execution is gated by a sophisticated approval system:

### Approval Policies

```typescript
export enum ApprovalPolicy {
  /**
   * Always ask for approval, for every command.
   */
  ALWAYS_ASK = "always-ask",

  /**
   * Only ask for approval for commands that might be destructive.
   */
  AUTO_APPROVE_SAFE = "auto-approve-safe",

  /**
   * Auto approve all commands without asking for permission (except specifically blacklisted ones).
   */
  AUTO_APPROVE_ALL = "auto-approve-all",
}
```

### Command Safety Classification

Commands are classified into different safety categories:

1. **Auto-approved**: Safe commands like `ls`, `cat`, etc.
2. **Need Review**: Potentially destructive commands like `rm`
3. **Rejected**: Blacklisted commands that are never allowed

### Approval Decisions

Users can make the following decisions for tool executions:

```typescript
export enum ReviewDecision {
  /**
   * No, don't do that. Abort the current task.
   */
  NO_ABORT = "no_abort",

  /**
   * No, don't do that. But continue with the current task.
   */
  NO_CONTINUE = "no_continue",

  /**
   * Yes, do that now.
   */
  YES = "yes",

  /**
   * Yes, and always do commands like this going forward.
   */
  ALWAYS = "always",

  /**
   * Request an explanation of what this command will do before deciding.
   */
  EXPLAIN = "explain",
}
```

## Sandboxing

Codex implements platform-specific sandboxing to enhance security:

```typescript
export enum SandboxType {
  /**
   * No sandboxing, execute commands directly.
   */
  NONE = "none",

  /**
   * macOS Seatbelt sandbox.
   */
  MACOS_SEATBELT = "macos-seatbelt",
}
```

The macOS Seatbelt implementation (`src/utils/agent/sandbox/macos-seatbelt.ts`) provides:

1. Process isolation
2. Restricted file system access
3. Network limitations
4. System call restrictions

## Tool Output Handling

When a tool is executed, its output is formatted and returned to the model in a structured format:

```typescript
function convertSummaryToResult(
  summary: ExecCommandSummary,
): HandleExecCommandResult {
  const { stdout, stderr, exitCode, durationMs } = summary;
  return {
    outputText: stdout || stderr,
    metadata: {
      exit_code: exitCode,
      duration_seconds: Math.round(durationMs / 100) / 10,
    },
  };
}
```

This structure helps the model understand:
- Whether the command succeeded (via exit code)
- The command's output
- How long the command took to execute

## Command Caching

To improve user experience, Codex implements a command approval caching system:

```typescript
const alwaysApprovedCommands = new Set<string>();

// Generate a stable key for command approval caching
function deriveCommandKey(cmd: Array<string>): string {
  const [maybeShell, maybeFlag, coreInvocation] = cmd;

  if (coreInvocation?.startsWith("apply_patch")) {
    return "apply_patch";
  }

  if (maybeShell === "bash" && maybeFlag === "-lc") {
    const script = coreInvocation ?? "";
    return script.split(/\s+/)[0] || "bash";
  }

  if (coreInvocation) {
    return coreInvocation.split(/\s+/)[0]!;
  }

  return JSON.stringify(cmd);
}
```

This allows the system to remember which commands the user has chosen to "always approve" during a session, reducing approval fatigue.

## Tool Limitations

The current tool system has several limitations:

1. **Limited Tool Set**: Only shell commands and file patching are supported
2. **No Direct File Editing**: Files can only be modified through the patch system
3. **No Network API Access**: No direct API calls to external services
4. **Bounded Environment**: Tools are constrained to the local environment
5. **Platform-Specific Sandboxing**: Not all platforms have full sandbox support

## Design Considerations

1. **Security First**: The tool system prioritizes security over convenience
2. **Structured Interface**: Tools use schema-validated arguments
3. **User Control**: All potentially destructive operations require approval
4. **Feedback Loop**: Tool output is always returned to the model
5. **Error Handling**: Robust error capture and reporting