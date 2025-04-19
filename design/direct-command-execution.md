# Direct Command Execution for Codex CLI

## Overview
Add a direct command execution mode to Codex CLI that allows users to run shell commands directly without AI processing, using a simple prefix.

## Motivation
Users frequently need to run simple, known commands (like `ls`, `pwd`, `cd`) without waiting for AI processing or approval workflows. This feature would:
- Save time for common operations
- Reduce token usage
- Provide a more integrated terminal experience

## Design

### User Interface
Use a simple prefix to indicate direct execution:
```
!ls -la
```
or
```
$pwd
```

### Implementation Approach

The key is to intercept these prefixed commands early in the processing pipeline while working within the existing architecture:

1. **Command Detection**:
   - Add logic in `src/cli.tsx` to detect prefixed commands in user input
   - For matching inputs, bypass the normal AI request flow

2. **Execution Path**:
   - Reuse the existing command execution infrastructure
   - Create a direct command handler that formats the command into the same structure expected by the execution pipeline

### Minimal API Impact

To minimize changes to the internal API design (#5):

```typescript
// src/utils/direct-command.ts
import type { CommandConfirmation } from "./agent/agent-loop.js";
import type { ApplyPatchCommand } from "../approvals.js";
import { handleExecCommand } from "./agent/handle-exec-command.js";
import { ReviewDecision } from "./agent/review.js";

export async function handleDirectCommand(
  rawCommand: string,
  config: AppConfig
): Promise<string> {
  // Strip the prefix (! or $)
  const command = rawCommand.slice(1).trim();
  
  // Split into argument array (handles quotes, etc.)
  const args = parseCommandIntoArgs(command);
  
  // Use either auto-approval or standard approval flow based on configuration
  const getApproval = (config: AppConfig) => {
    // Check if user has explicitly opted in to auto-approval for direct commands
    const autoApprove = config.directCommands?.autoApprove === true;
    
    if (autoApprove) {
      // Auto-approve if user has explicitly enabled this in config
      return async (command: Array<string>, applyPatch?: ApplyPatchCommand): 
        Promise<CommandConfirmation> => {
        return {
          review: ReviewDecision.YES,
          command
        };
      };
    } else {
      // Otherwise use the standard approval flow
      // This will prompt the user for confirmation
      return getCommandConfirmation;
    }
  };
  
  // Use the existing execution path
  const result = await handleExecCommand(
    { cmd: args },
    config,
    'auto-edit', // Use existing approval mode
    [], // No additional writable roots
    mockApproval
  );
  
  return result.outputText;
}

// Utility to parse command string into args array
function parseCommandIntoArgs(command: string): string[] {
  // Use shell-quote or similar to handle quotes, escapes, etc.
  // ...
}
```

This approach:
1. Uses the existing execution pipeline
2. Doesn't change core APIs
3. Only intercepts the commands at the input stage
4. Still respects overall system configuration

The direct command would flow through the normal execution path, but with an auto-approval, maintaining compatibility with the rest of the codebase while providing the requested functionality.

### Component Integration

1. **Terminal Input Handling**:
   ```typescript
   // In src/components/chat/terminal-chat-input.tsx
   
   // Handle special key prefixes
   if (value.startsWith('!') || value.startsWith('$')) {
     // Call direct command handler instead of sending to AI
     const output = await handleDirectCommand(value, config);
     addResponse({ type: 'direct_command', text: output });
     return;
   }
   
   // Continue with normal AI processing for other inputs
   ```

2. **Display in UI**:
   ```tsx
   // In src/components/chat/terminal-chat-response-item.tsx
   
   // Add a new response type for direct commands
   if (item.type === 'direct_command') {
     return (
       <Box>
         <Text color="gray">$ {item.originalCommand}</Text>
         <Text>{item.text}</Text>
       </Box>
     );
   }
   ```

## Security Considerations

1. **User Intent**: Commands with the prefix are explicitly intended to run directly, so normal safety checks are bypassed intentionally
2. **Execution Environment**: Commands still run in the same environment as AI-generated commands
3. **Visibility**: UI clearly indicates when a command was executed directly

## Testing Plan

1. Test direct execution of basic commands (`!ls`, `$pwd`)
2. Test commands with arguments and quotes
3. Test interactions with the current working directory
4. Test error handling and display

## Success Criteria

1. Users can run common shell commands directly with the prefix
2. By default, prefixed commands still require explicit user approval (no auto-execution)
3. Users must explicitly opt-in to automatic approval of direct commands via configuration
4. Output is displayed clearly in the terminal interface, distinguishing direct commands from AI-generated ones
5. The implementation doesn't break existing functionality

## Configuration Options

```yaml
# ~/.codex/config.yaml
directCommands:
  enabled: true  # Enable direct command recognition (default: true)
  autoApprove: false  # Auto-approve direct commands without confirmation (default: false)
  prefix: "!"  # Command prefix character(s) (default: "!")
```

Users must explicitly set `autoApprove: true` to bypass the approval workflow for direct commands. This ensures that the security model remains intact by default while providing power users the option to enable more efficient workflows.