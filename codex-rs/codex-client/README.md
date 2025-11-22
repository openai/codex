# codex-client

Generic transport layer that wraps HTTP requests, retries, and streaming primitives without any Codex/OpenAI awareness.

- Defines `HttpTransport` and a default `ReqwestTransport` plus thin `Request`/`Response` types.
- Provides retry utilities (`RetryPolicy`, `RetryOn`, `run_with_retry`, `backoff`) that callers plug into for unary and streaming calls.
- Supplies the `sse_stream` helper to turn byte streams into raw SSE `data:` frames with idle timeouts and surfaced stream errors.
- Consumed by higher-level crates like `codex-api`; it stays neutral on endpoints, headers, or API-specific error shapes.
