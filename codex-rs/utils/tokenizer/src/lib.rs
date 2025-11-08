use std::fmt;

use anyhow::Context;
use anyhow::Error as AnyhowError;
use thiserror::Error;
use tiktoken_rs::CoreBPE;

/// Supported local encodings.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum EncodingKind {
    O200kBase,
    Cl100kBase,
}

impl fmt::Display for EncodingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::O200kBase => f.write_str("o200k_base"),
            Self::Cl100kBase => f.write_str("cl100k_base"),
        }
    }
}

/// Tokenizer error type.
#[derive(Debug, Error)]
pub enum TokenizerError {
    #[error("failed to load encoding {kind}")]
    LoadEncoding {
        kind: EncodingKind,
        #[source]
        source: AnyhowError,
    },
    #[error("failed to decode tokens")]
    Decode {
        #[source]
        source: AnyhowError,
    },
}

/// Thin wrapper around a `tiktoken_rs::CoreBPE` tokenizer.
#[derive(Clone)]
pub struct Tokenizer {
    inner: CoreBPE,
}

impl Tokenizer {
    /// Build a tokenizer for a specific encoding.
    pub fn new(kind: EncodingKind) -> Result<Self, TokenizerError> {
        let loader: fn() -> anyhow::Result<CoreBPE> = match kind {
            EncodingKind::O200kBase => tiktoken_rs::o200k_base,
            EncodingKind::Cl100kBase => tiktoken_rs::cl100k_base,
        };

        let inner = loader().map_err(|source| TokenizerError::LoadEncoding { kind, source })?;
        Ok(Self { inner })
    }

    /// Default to `O200kBase`
    pub fn try_default() -> Result<Self, TokenizerError> {
        Self::new(EncodingKind::O200kBase)
    }

    /// Build a tokenizer using an `OpenAI` model name (maps to an encoding).
    /// Falls back to the `O200kBase` encoding when the model is unknown.
    pub fn for_model(model: &str) -> Result<Self, TokenizerError> {
        match tiktoken_rs::get_bpe_from_model(model) {
            Ok(inner) => Ok(Self { inner }),
            Err(model_error) => {
                let inner = tiktoken_rs::o200k_base()
                    .with_context(|| {
                        format!("fallback after model lookup failure for {model}: {model_error}")
                    })
                    .map_err(|source| TokenizerError::LoadEncoding {
                        kind: EncodingKind::O200kBase,
                        source,
                    })?;
                Ok(Self { inner })
            }
        }
    }

    /// Encode text to token IDs. If `with_special_tokens` is true, special
    /// tokens are allowed and may appear in the result.
    #[must_use]
    pub fn encode(&self, text: &str, with_special_tokens: bool) -> Vec<i32> {
        let raw = if with_special_tokens {
            self.inner.encode_with_special_tokens(text)
        } else {
            self.inner.encode_ordinary(text)
        };
        raw.into_iter().map(|t| t as i32).collect()
    }

    /// Count tokens in `text` as a signed integer.
    #[must_use]
    pub fn count(&self, text: &str) -> i64 {
        // Signed length to satisfy our style preference.
        i64::try_from(self.inner.encode_ordinary(text).len()).unwrap_or(i64::MAX)
    }

    /// Decode token IDs back to text.
    pub fn decode(&self, tokens: &[i32]) -> Result<String, TokenizerError> {
        let raw: Vec<u32> = tokens.iter().map(|t| *t as u32).collect();
        self.inner
            .decode(raw)
            .map_err(|source| TokenizerError::Decode { source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn cl100k_base_roundtrip_simple() -> Result<(), TokenizerError> {
        let tok = Tokenizer::new(EncodingKind::Cl100kBase)?;
        let s = "hello world";
        let ids = tok.encode(s, false);
        // Stable expectation for cl100k_base
        assert_eq!(ids, vec![15339, 1917]);
        let back = tok.decode(&ids)?;
        assert_eq!(back, s);
        Ok(())
    }

    #[test]
    fn preserves_whitespace_and_special_tokens_flag() -> Result<(), TokenizerError> {
        let tok = Tokenizer::new(EncodingKind::Cl100kBase)?;
        let s = "This  has   multiple   spaces";
        let ids_no_special = tok.encode(s, false);
        let round = tok.decode(&ids_no_special)?;
        assert_eq!(round, s);

        // With special tokens allowed, result may be identical for normal text,
        // but the API should still function.
        let ids_with_special = tok.encode(s, true);
        let round2 = tok.decode(&ids_with_special)?;
        assert_eq!(round2, s);
        Ok(())
    }

    #[test]
    fn model_mapping_builds_tokenizer() -> Result<(), TokenizerError> {
        // Choose a long-standing model alias that maps to cl100k_base.
        let tok = Tokenizer::for_model("gpt-5")?;
        let ids = tok.encode("ok", false);
        let back = tok.decode(&ids)?;
        assert_eq!(back, "ok");
        Ok(())
    }

    #[test]
    fn unknown_model_defaults_to_o200k_base() -> Result<(), TokenizerError> {
        let fallback = Tokenizer::new(EncodingKind::O200kBase)?;
        let tok = Tokenizer::for_model("does-not-exist")?;
        let text = "fallback please";
        assert_eq!(tok.encode(text, false), fallback.encode(text, false));
        Ok(())
    }
}
