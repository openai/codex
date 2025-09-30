# Codex SDK

Bring the power of the best coding agent to your application.

## Installation

```bash
npm install @openai/codex-sdk
```

## Usage

Call `startThread()` and `run()` to start a thead with Codex.

```typescript
import { Codex } from "@openai/codex-sdk";

const codex = new Codex();
const thread = codex.startThread();
const result = await thread.run("Diagnose the test failure and propose a fix");

console.log(result);
```

You can call `run()` again to continue the same thread.

```typescript
const result = await thread.run("Implement the fix");

console.log(result);
```

### Streaming

The `await run()` method completes when a thread turn is complete and agent is prepared the final response.

You can thread items while they are being produced by calling `await runStreamed()`.

```typescript
const result = thread.runStreamed("Diagnose the test failure and propose a fix");
```

### Resuming a thread

If you don't have the original `Thread` instance to continue the thread, you can resume a thread by calling `resumeThread()` and providing the thread.

```typescript
const threadId = "...";
const thread = codex.resumeThread(threadId);
const result = await thread.run("Implement the fix");

console.log(result);
```
