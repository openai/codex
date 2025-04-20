# Agent Flow in Codex

## Overview

Codex implements a single-agent conversational workflow with tool-calling capabilities. The agent flow follows an orchestrated pattern of user input, model processing, tool execution, and model continuation until the task is complete.

## Core Agent Loop

The agent loop is defined in `src/utils/agent/agent-loop.ts` and serves as the central orchestration component for Codex's agent system. It manages:

1. State of the conversation
2. API communication with OpenAI
3. Tool call execution
4. Error handling and retries
5. User interruption

## Flow Sequence Diagram

```
┌────────┐        ┌────────────┐      ┌──────────────┐        ┌───────────┐        ┌──────────┐
│  User  │        │ CLI (React)│      │  Agent Loop  │        │ OpenAI API│        │Tool System│
└───┬────┘        └─────┬──────┘      └──────┬───────┘        └─────┬─────┘        └────┬─────┘
    │                   │                    │                      │                    │
    │ Input Query       │                    │                      │                    │
    │───────────────────>                    │                      │                    │
    │                   │                    │                      │                    │
    │                   │ Send to Agent Loop │                      │                    │
    │                   │───────────────────>│                      │                    │
    │                   │                    │                      │                    │
    │                   │                    │ API Request          │                    │
    │                   │                    │─────────────────────>│                    │
    │                   │                    │                      │                    │
    │                   │                    │     Stream Response   │                    │
    │                   │                    │<─────────────────────│                    │
    │                   │                    │                      │                    │
    │                   │ Update UI          │                      │                    │
    │                   │<───────────────────│                      │                    │
    │                   │                    │                      │                    │
    │                   │                    │ If function_call     │                    │
    │                   │                    │──────────────────────┼─────────────────────>
    │                   │                    │                      │                    │
    │                   │                    │ Command Approval     │                    │
    │<─────────────────────────────────────────────────────────────┼────────────────────│
    │                   │                    │                      │                    │
    │ Approval Decision │                    │                      │                    │
    │───────────────────────────────────────>│                      │                    │
    │                   │                    │                      │                    │
    │                   │                    │ Execute Command      │                    │
    │                   │                    │──────────────────────┼─────────────────────>
    │                   │                    │                      │                    │
    │                   │                    │ Command Output       │                    │
    │                   │                    │<─────────────────────┼─────────────────────│
    │                   │                    │                      │                    │
    │                   │                    │ API Request with     │                    │
    │                   │                    │ Command Output       │                    │
    │                   │                    │─────────────────────>│                    │
    │                   │                    │                      │                    │
    │                   │                    │ Continue Response    │                    │
    │                   │                    │<─────────────────────│                    │
    │                   │                    │                      │                    │
    │                   │ Update UI          │                      │                    │
    │                   │<───────────────────│                      │                    │
    │                   │                    │                      │                    │
    │ View Response     │                    │                      │                    │
    │<──────────────────│                    │                      │                    │
    │                   │                    │                      │                    │
```

## Detailed Flow Steps

1. **User Input**
   - User provides natural language input through terminal
   - CLI captures and processes the input

2. **Agent Loop Initialization**
   - Agent Loop is instantiated or reused from previous interaction
   - Input is formatted and prepared for the model

3. **API Request to OpenAI**
   - Agent Loop sends request to OpenAI API
   - Includes conversation history, user input, and system instructions
   - Configures streaming for real-time response

4. **Processing Model Responses**
   - Model may respond with text content (displayed to the user)
   - Model may respond with function calls (processed by the agent)

5. **Tool Execution**
   - When function calls are received, the agent:
     - Validates function call arguments
     - Checks against approval policy
     - Seeks user approval if needed
     - Executes the command in appropriate sandbox
     - Captures output and errors

6. **Command Output Processing**
   - Command output is formatted and sent back to the model
   - Model receives the output and continues its reasoning

7. **Loop Continuation**
   - Process repeats until model completes the response
   - Agent continues to handle function calls until completion

8. **Cancellation and Error Handling**
   - User can cancel the current operation at any time
   - Network errors are retried with exponential backoff
   - Rate limits are handled with waiting and retry logic

## Key Implementation Details

### Function Call Handling

```typescript
private async handleFunctionCall(
  item: ResponseFunctionToolCall,
): Promise<Array<ResponseInputItem>> {
  // Normalize function call format
  const name = isChatStyle ? (item as any).function?.name : (item as any).name;
  const rawArguments = isChatStyle ? (item as any).function?.arguments : (item as any).arguments;
  const callId = (item as any).call_id ?? (item as any).id;
  
  // Parse arguments
  const args = parseToolCallArguments(rawArguments ?? "{}");
  
  // Currently supports shell/container.exec 
  if (name === "container.exec" || name === "shell") {
    const { outputText, metadata, additionalItems } = await handleExecCommand(
      args,
      this.config,
      this.approvalPolicy,
      this.additionalWritableRoots,
      this.getCommandConfirmation,
      this.execAbortController?.signal,
    );
    
    // Format output for the model
    outputItem.output = JSON.stringify({ output: outputText, metadata });
    
    // Handle any additional items that need to be sent to the model
    if (additionalItems) {
      additionalItems.push(...additionalItemsFromExec);
    }
  }
  
  return [outputItem, ...additionalItems];
}
```

### Command Approval System

```typescript
async function askUserPermission(
  args: ExecInput,
  applyPatchCommand: ApplyPatchCommand | undefined,
  getCommandConfirmation: (
    command: Array<string>,
    applyPatch: ApplyPatchCommand | undefined,
  ) => Promise<CommandConfirmation>,
): Promise<HandleExecCommandResult | null> {
  const { review: decision, customDenyMessage } = 
    await getCommandConfirmation(args.cmd, applyPatchCommand);

  if (decision === ReviewDecision.ALWAYS) {
    // Remember this command to avoid future prompts
    const key = deriveCommandKey(args.cmd);
    alwaysApprovedCommands.add(key);
  }

  // Handle explanation request (continue with normal flow)
  if (decision === ReviewDecision.EXPLAIN) {
    return null;
  }

  // Handle rejection (abort execution with message)
  if (decision !== ReviewDecision.YES && decision !== ReviewDecision.ALWAYS) {
    const note = decision === ReviewDecision.NO_CONTINUE
      ? customDenyMessage?.trim() || "No, don't do that — keep going though."
      : "No, don't do that — stop for now.";
    
    return {
      outputText: "aborted",
      metadata: {},
      additionalItems: [
        {
          type: "message",
          role: "user",
          content: [{ type: "input_text", text: note }],
        },
      ],
    };
  }
  
  // User approved (continue with normal flow)
  return null;
}
```

## Agent System Resilience

The Codex agent system incorporates several resilience features:

1. **Network Retry Logic**:
   - Retries on transient errors with exponential backoff
   - Special handling for rate limit errors
   - Timeout recovery

2. **Cancellation Management**:
   - Can cancel in-progress model responses
   - Can cancel in-progress tool executions
   - Handles cleanup of pending requests

3. **Error Recovery**:
   - Recovers from API errors
   - Provides user-friendly error messages
   - Can continue despite temporary failures

4. **Model Response Validation**:
   - Validates function call arguments
   - Handles malformed responses
   - Protects against invalid commands

## Unique Insights

1. **Single Agent Architecture**: Codex uses a single agent rather than a multi-agent approach, maintaining a continuous conversation state with the model.

2. **Stateful Function Calls**: The system tracks "pending" function calls to handle cancellations correctly and satisfy the OpenAI API contract.

3. **Approval Caching**: Uses a session-level cache to remember command approvals, improving UX by reducing repeated prompts.

4. **Platform-aware Sandboxing**: Employs different sandbox strategies based on the operating system.

5. **Progressive Response Delivery**: Uses delayed staging of response items to provide a responsive feel while allowing for cancellation.