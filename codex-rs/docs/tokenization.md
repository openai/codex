**Tokenization Utilities**

- Crate: `codex-utils-tokenizer`
- Purpose: Local tokenization/detokenization using OpenAI’s tiktoken encodings.

The implementation wraps the `tiktoken-rs` crate so we can encode/decode fully offline. This crate bundles encoding data (e.g., `cl100k_base`) and does not require network access at runtime.

Model helpers:
- `Tokenizer::for_model(model)` resolves the encoding used by an OpenAI
  deployment. Unknown models automatically fall back to the `o200k_base`
  tokenizer so local workflows keep functioning even if new model aliases
  appear.

Example:
- Encode and decode with cl100k_base

```rust
use codex_utils_tokenizer::{EncodingKind, Tokenizer};

let tok = Tokenizer::new(EncodingKind::Cl100kBase)?;
let ids = tok.encode("hello world", false);
assert_eq!(ids, vec![15339, 1917]);
let text = tok.decode(&ids)?;
assert_eq!(text, "hello world");
```
