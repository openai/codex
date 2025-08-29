# Add GitHub Models provider PR

Created at: 2025-08-29T07:42:38Z

## Plan
- Base from origin/main
- Remove .a5c/.github files on branch
- Include only core provider + docs
- Run fmt/lints/tests
- Open draft PR to openai/codex

## Results
- Provider added in codex-rs/core/src/model_provider_info.rs
- Docs updated in docs/config.md
- Tests pass (core, all-features) with network disabled
- Draft PR: https://github.com/openai/codex/pull/2889
