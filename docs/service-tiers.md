# Service Tiers

Codex supports requesting an optional "service tier" from model providers that expose this feature (for example, some OpenAI endpoints).

A service tier is an opaque string sent as a top-level `service_tier` parameter in Responses/Completions requests. Examples used in this codebase include:

- `priority` — higher priority routing for lower-latency or higher-throughput access
- `flex` — lower-cost or best-effort routing

Note: exact semantics and available tier names depend on the provider. If the provider does not support `service_tier`, the parameter is ignored.

Where to configure

- Per-installation default: set `model_service_tier` in your `$CODEX_HOME/config.toml` (see `docs/example-config.md`). This value is propagated to outgoing requests where no per-request tier is set.
- Per-request override: callers that build Responses/Completions requests can set `service_tier` on the request object; this overrides the configured default for that request.

Behavior in Codex

- The `model_service_tier` configuration key is optional. When set, Codex includes the value as the `service_tier` top-level parameter on Responses/Completions requests for providers that use the Responses API.
- The TUI displays the active service tier in the status card when it is configured.

Security and privacy

- Service tier values are treated as non-secret metadata and are not redacted. Do not use secrets or tokens as tier values.

Examples

1. Configuring a default in `~/.codex/config.toml`:

```toml
# Request the "priority" tier by default for responses/completions
model_service_tier = "priority"
```

2. Overriding per-request (high-level example):

```rust
let req = ResponsesRequest::new(...).service_tier(Some("flex".to_string()));
```

When in doubt, consult your model provider's documentation for supported service tier names and semantics.

