# Security & Sandboxing in Codex

## Overview

Codex implements a comprehensive security model to safely execute model-generated code and commands. This is crucial for an AI coding assistant that can run arbitrary shell commands and modify files. The security system is built on several layers of protection:

1. **Command Approval System**: User-facing approval workflow for commands
2. **Sandboxed Execution**: Platform-specific isolation for command execution
3. **Access Control**: Path-based control for file modifications
4. **Input Validation**: Strict validation of tool arguments

## Command Approval System

### Approval Policies

Codex implements three approval policies defined in `approvals.ts`:

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

### Safety Classification

Commands are classified into different safety categories in the `canAutoApprove` function:

```typescript
export function canAutoApprove(
  command: Array<string>,
  policy: ApprovalPolicy,
  writableRoots: ReadonlyArray<string> = [],
): ApprovalResult {
  // Check for special case commands like apply_patch
  if (isApplyPatchCommand(command)) {
    const patch = extractPatch(command);
    // Analyze the patch to determine safety
    // ...
  }

  // Check against safe command whitelist
  if (isSafeCommand(command)) {
    return {
      type: "auto-approve",
      runInSandbox: policy !== ApprovalPolicy.AUTO_APPROVE_ALL,
    };
  }

  // Check against explicitly rejected commands
  if (isExplicitlyRejectedCommand(command)) {
    return { type: "reject" };
  }

  // For all other commands, follow the policy
  if (policy === ApprovalPolicy.AUTO_APPROVE_ALL) {
    return { type: "auto-approve", runInSandbox: false };
  }

  // Default to asking the user
  return { type: "ask-user" };
}
```

### User Approval Interface

When user approval is required, a confirmation UI is presented:

```typescript
async function askUserPermission(
  args: ExecInput,
  applyPatchCommand: ApplyPatchCommand | undefined,
  getCommandConfirmation: (
    command: Array<string>,
    applyPatch: ApplyPatchCommand | undefined,
  ) => Promise<CommandConfirmation>,
): Promise<HandleExecCommandResult | null> {
  const { review: decision, customDenyMessage } = await getCommandConfirmation(
    args.cmd,
    applyPatchCommand,
  );

  // Handle decision cases (ALWAYS, YES, NO, etc.)
  // ...
}
```

## Sandboxed Execution

### Sandbox Types

Codex defines different sandbox types in `sandbox/interface.ts`:

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

### Platform-Specific Implementation

The sandbox implementation is platform-specific:

```typescript
async function getSandbox(runInSandbox: boolean): Promise<SandboxType> {
  if (runInSandbox) {
    if (process.platform === "darwin") {
      return SandboxType.MACOS_SEATBELT;
    } else if (await isInLinux()) {
      return SandboxType.NONE;
    } else if (process.platform === "win32") {
      // On Windows, we don't have a sandbox implementation yet
      log(
        "WARNING: Sandbox was requested but is not available on Windows. Continuing without sandbox.",
      );
      return SandboxType.NONE;
    }
    // For other platforms, still throw an error as before
    throw new Error("Sandbox was mandated, but no sandbox is available!");
  } else {
    return SandboxType.NONE;
  }
}
```

### macOS Seatbelt Implementation

For macOS, Codex uses the macOS Seatbelt sandbox to provide strong isolation for executed commands:

```typescript
// Example from macos-seatbelt.ts
export async function sandboxExec(
  exec: ExecInput,
  signal?: AbortSignal,
): Promise<ExecOutput> {
  const { cmd, workdir, timeoutInMillis } = exec;
  
  // Build a sandbox profile that:
  // - Allows read access to most of the file system
  // - Restricts write access to specific paths
  // - Prevents network access
  // - Limits system calls
  
  const sandboxProfile = buildSeatbeltProfile(writablePaths);
  
  // Launch the command within the sandbox
  return launchWithSeatbelt(cmd, sandboxProfile, workdir, timeoutInMillis, signal);
}

function buildSeatbeltProfile(writablePaths: string[]): string {
  // Create a seatbelt profile that:
  // 1. Denies all by default
  // 2. Allows specific read operations
  // 3. Only allows writes to specified paths
  // 4. Blocks network access
  // 5. Allows necessary system calls
  
  return `
    (version 1)
    (deny default)
    (allow file-read*)
    (deny file-write* (subpath "/"))
    ${writablePaths.map(p => `(allow file-write* (subpath "${p}"))`).join('\n')}
    (deny network*)
    (allow process-exec)
    (allow system-socket)
    ... other allowed operations ...
  `;
}
```

## Access Control

### Writable Paths Restriction

Codex implements path-based access control to limit where file modifications can occur:

```typescript
// Example of writable paths handling
export async function handleExecCommand(
  args: ExecInput,
  config: AppConfig,
  policy: ApprovalPolicy,
  additionalWritableRoots: ReadonlyArray<string>,
  getCommandConfirmation: (
    command: Array<string>,
    applyPatch: ApplyPatchCommand | undefined,
  ) => Promise<CommandConfirmation>,
  abortSignal?: AbortSignal,
): Promise<HandleExecCommandResult> {
  // Default to current working directory if no additional paths specified
  const writablePaths = [process.cwd(), ...additionalWritableRoots];
  
  // Check if command is allowed to write to these paths
  const safety = canAutoApprove(command, policy, writablePaths);
  
  // Rest of the command execution logic
  // ...
}
```

### Patch Safety Analysis

For file modifications via patches, Codex analyzes the patch content to ensure safety:

```typescript
function analyzeFileModifications(patch: string): PatchAnalysisResult {
  // Parse the patch to extract file paths
  const filePaths = extractFilePaths(patch);
  
  // Check each path against allowed writable roots
  for (const filePath of filePaths) {
    if (!isPathWithinAllowedRoots(filePath, writableRoots)) {
      return {
        safe: false,
        reason: `Patch attempts to modify file outside allowed paths: ${filePath}`,
      };
    }
  }
  
  // Check for sensitive file patterns
  if (containsSensitiveFiles(filePaths)) {
    return {
      safe: false,
      reason: "Patch attempts to modify sensitive files",
    };
  }
  
  return { safe: true };
}
```

## Input Validation

### Tool Argument Validation

All tool calls undergo strict argument validation:

```typescript
// From agent-loop.ts
const args = parseToolCallArguments(rawArguments ?? "{}");
log(
  `handleFunctionCall(): name=${
    name ?? "undefined"
  } callId=${callId} args=${rawArguments}`,
);

if (args == null) {
  const outputItem: ResponseInputItem.FunctionCallOutput = {
    type: "function_call_output",
    call_id: item.call_id,
    output: `invalid arguments: ${rawArguments}`,
  };
  return [outputItem];
}
```

### Command Structure Validation

Commands are validated for structure and safety:

```typescript
// Validation of shell commands
function validateShellCommand(command: Array<string>): ValidationResult {
  if (!Array.isArray(command) || command.length === 0) {
    return {
      valid: false,
      reason: "Command must be a non-empty array",
    };
  }
  
  // Check for disallowed commands
  const commandName = command[0].toLowerCase();
  if (BLACKLISTED_COMMANDS.includes(commandName)) {
    return {
      valid: false,
      reason: `Command '${commandName}' is not allowed`,
    };
  }
  
  return { valid: true };
}
```

## Session Memory and Approval Caching

Codex implements a session-based approval caching mechanism:

```typescript
// From handle-exec-command.ts
const alwaysApprovedCommands = new Set<string>();

function deriveCommandKey(cmd: Array<string>): string {
  // Generate a stable key that ignores volatile parts
  // ...
}

// In user approval handling
if (decision === ReviewDecision.ALWAYS) {
  // Persist this command so we won't ask again during this session.
  const key = deriveCommandKey(args.cmd);
  alwaysApprovedCommands.add(key);
}
```

This improves UX by remembering which command patterns the user has previously approved.

## Integration with Agent Loop

Security measures are tightly integrated with the agent loop execution flow:

```typescript
// Simplified integration flow
private async handleFunctionCall(
  item: ResponseFunctionToolCall,
): Promise<Array<ResponseInputItem>> {
  // 1. Validate function call
  // 2. Check approval policy
  // 3. Get user confirmation if needed
  // 4. Execute in sandbox if required
  // 5. Return results to model
  
  if (name === "container.exec" || name === "shell") {
    const {
      outputText,
      metadata,
      additionalItems: additionalItemsFromExec,
    } = await handleExecCommand(
      args,
      this.config,
      this.approvalPolicy,
      this.additionalWritableRoots,
      this.getCommandConfirmation,
      this.execAbortController?.signal,
    );
    outputItem.output = JSON.stringify({ output: outputText, metadata });
    
    if (additionalItemsFromExec) {
      additionalItems.push(...additionalItemsFromExec);
    }
  }
  
  return [outputItem, ...additionalItems];
}
```

## Security Insights and Design Decisions

### Core Security Principles

1. **Defense in Depth**: Multiple security layers work together
2. **Least Privilege**: Commands only get access to what they need
3. **User Control**: Critical operations require explicit approval
4. **Safe Defaults**: Default policies prioritize security
5. **Platform-Aware**: Security adapts to the underlying OS

### Platform-Specific Considerations

- **macOS**: Leverages Seatbelt for strong sandboxing
- **Windows**: Currently lacks sandbox implementation
- **Linux**: Basic sandboxing support planned but not fully implemented

### Limitations of Current Approach

1. **Platform Coverage**: Not all platforms have equal sandbox protection
2. **Command Granularity**: Some complex commands may have mixed safety properties
3. **Approval Fatigue**: Users may experience fatigue with frequent approval requests
4. **Network Restrictions**: Limited ability to safely allow network access
5. **Persistent Threats**: No protection against persistent backdoors inserted in edits

### Future Security Enhancements

Potential areas for improved security:

1. **Static Analysis**: Pre-analyze code edits for security implications
2. **Enhanced Sandboxing**: Implement Windows and Linux sandboxing
3. **Fine-grained Permissions**: More nuanced permission model beyond read/write
4. **Security Policy Customization**: Allow users to define custom security policies
5. **Audit Logging**: Track security-relevant operations for review