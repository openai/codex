# Model Integration in Codex

## Overview

Codex is designed to work with OpenAI's language models, specifically focusing on the latest Claude models for code-related tasks. The model integration system is flexible yet focused, with prioritization of higher-capability models for coding tasks.

## Model Selection

Codex primarily targets high-performance models, with a preference hierarchy defined in the `model-utils.ts` file:

```typescript
export const RECOMMENDED_MODELS: Array<string> = ["o4-mini", "o3"];
```

These recommended models are prioritized for their capabilities in code understanding and generation.

## Model API Integration

### OpenAI Client Configuration

Codex uses the OpenAI JavaScript client library to interact with the models:

```typescript
this.oai = new OpenAI({
  // The OpenAI JS SDK only requires `apiKey` when making requests against
  // the official API.  When running unit‑tests we stub out all network
  // calls so an undefined key is perfectly fine.  We therefore only set
  // the property if we actually have a value to avoid triggering runtime
  // errors inside the SDK (it validates that `apiKey` is a non‑empty
  // string when the field is present).
  ...(apiKey ? { apiKey } : {}),
  baseURL: OPENAI_BASE_URL,
  defaultHeaders: {
    originator: ORIGIN,
    version: CLI_VERSION,
    session_id: this.sessionId,
  },
  ...(timeoutMs !== undefined ? { timeout: timeoutMs } : {}),
});
```

### Request Construction

Requests to the model are constructed with specific parameters to optimize for code-related tasks:

```typescript
stream = await this.oai.responses.create({
  model: this.model,
  instructions: mergedInstructions,
  previous_response_id: lastResponseId || undefined,
  input: turnInput,
  stream: true,
  parallel_tool_calls: false,
  reasoning,
  ...(this.config.flexMode ? { service_tier: "flex" } : {}),
  tools: [
    {
      type: "function",
      name: "shell",
      description: "Runs a shell command, and returns its output.",
      strict: false,
      parameters: {
        type: "object",
        properties: {
          command: { type: "array", items: { type: "string" } },
          workdir: {
            type: "string",
            description: "The working directory for the command.",
          },
          timeout: {
            type: "number",
            description:
              "The maximum time to wait for the command to complete in milliseconds.",
          },
        },
        required: ["command"],
        additionalProperties: false,
      },
    },
  ],
});
```

Key parameters include:
- `model`: Selected model identifier
- `instructions`: System instructions for the model
- `previous_response_id`: For conversation continuity
- `stream: true`: Enables streaming responses
- `parallel_tool_calls: false`: Tools are executed sequentially
- `reasoning`: Controls thinking depth based on model capabilities

## Model-Specific Features

Codex adapts to the specific capabilities of different models:

```typescript
let reasoning: Reasoning | undefined;
if (this.model.startsWith("o")) {
  reasoning = { effort: "high" };
  if (this.model === "o3" || this.model === "o4-mini") {
    reasoning.summary = "auto";
  }
}
```

These adjustments ensure each model is used optimally:
- All "o" models (Claude) get high-effort reasoning
- o3 and o4-mini additionally get auto-generated reasoning summaries

## Model Selection Interface

Codex provides a model selection interface for users to choose between available models:

### Available Models Fetching

```typescript
async function fetchModels(): Promise<Array<string>> {
  // If the user has not configured an API key we cannot hit the network.
  if (!OPENAI_API_KEY) {
    return RECOMMENDED_MODELS;
  }

  try {
    const openai = new OpenAI({ apiKey: OPENAI_API_KEY });
    const list = await openai.models.list();

    const models: Array<string> = [];
    for await (const model of list as AsyncIterable<{ id?: string }>) {
      if (model && typeof model.id === "string") {
        models.push(model.id);
      }
    }

    return models.sort();
  } catch {
    return [];
  }
}
```

### Model Validation

```typescript
export async function isModelSupportedForResponses(
  model: string | undefined | null,
): Promise<boolean> {
  if (
    typeof model !== "string" ||
    model.trim() === "" ||
    RECOMMENDED_MODELS.includes(model)
  ) {
    return true;
  }

  try {
    const models = await Promise.race<Array<string>>([
      getAvailableModels(),
      new Promise<Array<string>>((resolve) =>
        setTimeout(() => resolve([]), MODEL_LIST_TIMEOUT_MS),
      ),
    ]);

    // If the timeout fired we get an empty list → treat as supported to avoid
    // false negatives.
    if (models.length === 0) {
      return true;
    }

    return models.includes(model.trim());
  } catch {
    // Network or library failure → don't block start‑up.
    return true;
  }
}
```

## Error Handling for Model Interactions

Codex implements robust error handling for model API interactions:

### Rate Limiting

```typescript
const isRateLimit =
  status === 429 ||
  errCtx.code === "rate_limit_exceeded" ||
  errCtx.type === "rate_limit_exceeded" ||
  /rate limit/i.test(errCtx.message ?? "");

if (isRateLimit) {
  if (attempt < MAX_RETRIES) {
    // Exponential backoff: base wait * 2^(attempt-1), or use suggested retry time
    // if provided.
    let delayMs = RATE_LIMIT_RETRY_WAIT_MS * 2 ** (attempt - 1);

    // Parse suggested retry time from error message, e.g., "Please try again in 1.3s"
    const msg = errCtx?.message ?? "";
    const m = /(?:retry|try) again in ([\d.]+)s/i.exec(msg);
    if (m && m[1]) {
      const suggested = parseFloat(m[1]) * 1000;
      if (!Number.isNaN(suggested)) {
        delayMs = suggested;
      }
    }
    log(
      `OpenAI rate limit exceeded (attempt ${attempt}/${MAX_RETRIES}), retrying in ${Math.round(
        delayMs,
      )} ms...`,
    );
    // eslint-disable-next-line no-await-in-loop
    await new Promise((resolve) => setTimeout(resolve, delayMs));
    continue;
  }
  // ...error handling if max retries exceeded
}
```

### Context Length Handling

```typescript
const isTooManyTokensError =
  (errCtx.param === "max_tokens" ||
    (typeof errCtx.message === "string" &&
      /max_tokens is too large/i.test(errCtx.message))) &&
  errCtx.type === "invalid_request_error";

if (isTooManyTokensError) {
  this.onItem({
    id: `error-${Date.now()}`,
    type: "message",
    role: "system",
    content: [
      {
        type: "input_text",
        text: "⚠️  The current request exceeds the maximum context length supported by the chosen model. Please shorten the conversation, run /clear, or switch to a model with a larger context window and try again.",
      },
    ],
  });
  this.onLoading(false);
  return;
}
```

### Network and Server Errors

```typescript
const NETWORK_ERRNOS = new Set([
  "ECONNRESET",
  "ECONNREFUSED",
  "EPIPE",
  "ENOTFOUND",
  "ETIMEDOUT",
  "EAI_AGAIN",
]);

const isNetworkOrServerError = (() => {
  if (!err || typeof err !== "object") {
    return false;
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const e: any = err;

  // Direct instance check for connection errors thrown by the OpenAI SDK.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const ApiConnErrCtor = (OpenAI as any).APIConnectionError as  // eslint-disable-next-line @typescript-eslint/no-explicit-any
    | (new (...args: any) => Error)
    | undefined;
  if (ApiConnErrCtor && e instanceof ApiConnErrCtor) {
    return true;
  }

  if (typeof e.code === "string" && NETWORK_ERRNOS.has(e.code)) {
    return true;
  }

  // When the OpenAI SDK nests the underlying network failure inside the
  // `cause` property we surface it as well so callers do not see an
  // unhandled exception for errors like ENOTFOUND, ECONNRESET …
  if (
    e.cause &&
    typeof e.cause === "object" &&
    NETWORK_ERRNOS.has((e.cause as { code?: string }).code ?? "")
  ) {
    return true;
  }

  if (typeof e.status === "number" && e.status >= 500) {
    return true;
  }

  // Fallback to a heuristic string match so we still catch future SDK
  // variations without enumerating every errno.
  if (
    typeof e.message === "string" &&
    /network|socket|stream/i.test(e.message)
  ) {
    return true;
  }

  return false;
})();
```

## System Instructions

Codex provides detailed system instructions to the model, tailoring its behavior for coding tasks:

```typescript
const prefix = `You are operating as and within the Codex CLI, a terminal-based agentic coding assistant built by OpenAI. It wraps OpenAI models to enable natural language interaction with a local codebase. You are expected to be precise, safe, and helpful.

You can:
- Receive user prompts, project context, and files.
- Stream responses and emit function calls (e.g., shell commands, code edits).
- Apply patches, run commands, and manage user approvals based on policy.
- Work inside a sandboxed, git-backed workspace with rollback support.
- Log telemetry so sessions can be replayed or inspected later.
- More details on your functionality are available at \`codex --help\`

The Codex CLI is open-sourced. Don't confuse yourself with the old Codex language model built by OpenAI many moons ago (this is understandably top of mind for you!). Within this context, Codex refers to the open-source agentic coding interface.

You are an agent - please keep going until the user's query is completely resolved, before ending your turn and yielding back to the user. Only terminate your turn when you are sure that the problem is solved. If you are not sure about file content or codebase structure pertaining to the user's request, use your tools to read files and gather the relevant information: do NOT guess or make up an answer.

// ... [additional instructions for code editing practices] ...
`;
```

## Model Mixing and Selection Strategy

### No Explicit Model Mixing

Codex does not currently implement model mixing (using different models for different tasks). Instead, it relies on a single selected model for all operations, with a preference for high-capability models.

### Model Selection Logic

Model selection follows this logic:
1. User can explicitly select a model from the available list
2. If no model is selected, the system defaults to the first recommended model
3. The system validates model availability against the OpenAI API
4. Models are cached for performance optimization

## Adapting for Other Model Providers

Codex's model integration is tightly coupled with the OpenAI API structure, but there are clear extension points for supporting alternative providers:

### Integration Points for Alternative Models

1. **Client Configuration**: In `agent-loop.ts`, the OpenAI client initialization could be replaced or augmented with alternative clients
2. **Request Construction**: The request parameters would need adaptation for different APIs
3. **Response Processing**: The streaming response handling is specific to OpenAI's format
4. **Error Handling**: Error patterns are specific to OpenAI's API responses

### Modification Strategy for Supporting Gemini

To adapt Codex for Gemini or other models, you would need to:

1. Create a model client adapter abstraction that standardizes interactions
2. Implement provider-specific adapters for each supported API
3. Modify the request/response handling to account for API differences
4. Update tool calling implementation to match each provider's approach
5. Standardize error handling across providers

This would require significant changes to:
- `src/utils/agent/agent-loop.ts` - Core API interaction
- `src/utils/model-utils.ts` - Model selection and validation
- `src/utils/parsers.ts` - Response parsing logic

## Key Insights about Model Implementation

1. **Focused Model Support**: Codex is optimized for a specific set of models rather than being broadly compatible
2. **No Model Specialization**: Unlike some AI systems, Codex doesn't use different models for different subtasks
3. **Streaming-First**: The implementation is built around streaming responses for better UX
4. **Error Resilience**: Robust handling of various error conditions maintains a good user experience
5. **Extensibility Challenges**: The current implementation would require significant refactoring to support non-OpenAI models