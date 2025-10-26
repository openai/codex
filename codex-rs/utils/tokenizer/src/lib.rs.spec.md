## Overview
`codex_utils_tokenizer::lib` exposes the `EncodingKind` enum, a `Tokenizer` wrapper around `tiktoken_rs::CoreBPE`, and an error type that normalizes tokenizer failures for workspace consumers.

## Detailed Behavior
- `EncodingKind` covers the encodings Codex relies on (`o200k_base`, `cl100k_base`) and implements `Display` for human-readable errors.
- `Tokenizer::new(kind)` loads the requested encoding via the corresponding `tiktoken-rs` factory function, mapping loader failures into `TokenizerError::LoadEncoding`.
- `Tokenizer::for_model(model)` asks `tiktoken_rs` for the model-specific BPE. On failure it logs the model error in the context string and falls back to `o200k_base`, preserving a working tokenizer while still surfacing the issue via `TokenizerError::LoadEncoding` if the fallback cannot load.
- `encode(text, with_special_tokens)` encodes to signed `i32` IDs, toggling special-token support based on the flag.
- `count(text)` provides a signed count (preferred over unsigned values in Codex) by encoding the text and casting the length to `i64`, saturating at `i64::MAX` on overflow.
- `decode(tokens)` maps `i32` IDs back to text, converting to `u32` for `tiktoken` and wrapping decode failures in `TokenizerError::Decode`.
- Module tests verify round-tripping, whitespace preservation, model aliasing, and fallback behavior to guard against upstream library changes.

## Broader Context
- Designed for crates that need OpenAI-compatible token counting without each crate binding directly to `tiktoken-rs`. Context can't yet be determined for concrete integrations because no current code imports the wrapper; future spec updates should note actual consumers once adopted.

## Technical Debt
- None identified; error mapping and fallback logic already cover the known failure modes of `tiktoken-rs`.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
