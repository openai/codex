# Codex SDK

Bring the power of the best coding agent to your application.

## Installation

```bash
npm install @openai/codex-sdk
```

## Usage

Call `startThread()` and `run()` to start a thread with Codex.

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

The `run()` method completes when a thread turn is complete and the agent has produced the final response.

You can stream events while they are being produced by calling `runStreamed()` and iterating the returned generator.

```typescript
const { events } = await thread.runStreamed("Diagnose the test failure and propose a fix");

for await (const event of events) {
  console.log(event);
}
```

### Resuming a thread

If you don't have the original `Thread` instance to continue the thread, you can resume by calling `resumeThread()` and providing the thread identifier.

```typescript
const threadId = "...";
const thread = codex.resumeThread(threadId);
const result = await thread.run("Implement the fix");

console.log(result);
```

### Working directory

By default, Codex will run in the current working directory. You can change the working directory by passing the `workingDirectory` option when creating a thread.

```typescript
const thread = codex.startThread({
  workingDirectory: "/path/to/working/directory",
});
```

To avoid unrecoverable errors, Codex requires the working directory to be a Git repository. You can skip the Git repository check by passing the `skipGitRepoCheck` option when creating a thread.

```typescript
const thread = codex.startThread({
  skipGitRepoCheck: true,
});
```
