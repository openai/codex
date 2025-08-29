# Add GitHub Models provider

## Context
Implement built-in provider `github` to use GitHub Models via the Responses API.

## Plan
- Add `github` provider in `codex-rs/core/src/model_provider_info.rs`
- Default `base_url` = `https://models.inference.ai.azure.com`
- `env_key` = `GITHUB_TOKEN` (Bearer)
- `wire_api` = `responses`
- Docs: update `docs/config.md` with usage
- Verify: fmt, lint, tests (core, then workspace)

## Notes
- No changes to sandbox env vars.
- Skip provider-specific headers unless required.
