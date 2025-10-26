## Overview
`codex-utils-tokenizer` wraps the `tiktoken-rs` library to provide a stable workspace interface for encoding and decoding text with OpenAI-compatible tokenizers.

## Detailed Behavior
- Re-exports `Tokenizer`, `EncodingKind`, and `TokenizerError` from `src/lib.rs`, keeping the public surface focused on loading encodings and transforming token sequences.
- Depends on `tiktoken-rs` only, allowing other crates to rely on this wrapper without wiring `tiktoken` directly into their manifests.

## Broader Context
- Intended for components that need to enforce message or prompt budgets using the same token estimates as OpenAI’s API. Context can't yet be determined for specific consumers; no downstream crate currently imports it directly in-tree.

## Technical Debt
- None at the crate level; future adopters should verify error surfaces meet their UX requirements before exposing them to end users.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./src/lib.rs.spec.md
