# hyper-sdk Development Guide

## Independence Constraint

**CRITICAL**: hyper-sdk is an INDEPENDENT crate. It must NOT depend on:
- `cocode-protocol` or any `common/` crates
- Any crates outside `provider-sdks/`

hyper-sdk can ONLY depend on:
- Individual provider SDKs (anthropic-sdk, openai-sdk, google-genai-sdk, volcengine-ark-sdk, z-ai-sdk)
- Standard external crates (tokio, serde, reqwest, async-trait, etc.)

This ensures hyper-sdk remains a standalone, reusable multi-provider SDK.

## Key Types

| Type | Purpose |
|------|---------|
| `GenerateResponse` | Unified API return (stream/non-stream) |
| `Message` | History item with role, content, metadata |
| `StreamProcessor` | Streaming with snapshot-based updates |
| `ConversationContext` | Optional history management (simple use cases) |
| `ProviderMetadata` | Source tracking for cross-provider support |

## Cross-Provider Support

`Message.convert_for_provider(provider, model)` sanitizes messages for cross-provider compatibility:
- Strips thinking signatures when switching providers/models
- Preserves tool call IDs (critical for tool call correlation)
- Tracks source provider in metadata for debugging
- Clears provider-specific options that won't be understood

## Provider Implementations

Each provider is in `src/providers/`:
- `openai.rs` - OpenAI (Responses API)
- `anthropic.rs` - Anthropic Claude (Messages API)
- `gemini.rs` - Google Gemini (GenerateContent API)
- `volcengine.rs` - Volcengine Ark
- `zai.rs` - Z.AI / ZhipuAI
- `openai_compat.rs` - Generic OpenAI-compatible

## Development Workflow

```bash
# From codex/ directory (not cocode-rs/)
cargo check -p hyper-sdk --manifest-path cocode-rs/Cargo.toml
cargo test -p hyper-sdk --manifest-path cocode-rs/Cargo.toml
cargo build --manifest-path cocode-rs/Cargo.toml
```

## Adding New Provider Support

1. Create `src/providers/<provider>.rs`
2. Implement `Provider` trait
3. Add internal Model implementation
4. Implement message/tool conversion helpers
5. Add error mapping from provider SDK
6. Export from `src/providers/mod.rs`
7. Add to `src/lib.rs` re-exports

## Error Handling

Use `HyperError` variants:
- `ProviderNotFound` / `ModelNotFound` - Configuration errors
- `AuthenticationFailed` - API key issues
- `RateLimitExceeded` - Retryable (temporary)
- `QuotaExceeded` - NOT retryable (billing change needed)
- `ContextWindowExceeded` - Input too long
- `StreamIdleTimeout` - No events received
- `Retryable { message, delay }` - Generic retryable with delay hint

## Testing

Provider implementations should have basic unit tests for:
- Builder pattern construction
- Missing API key handling
- Message conversion helpers
