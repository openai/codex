//! Provider-facing usage comes from the same token report as the built-in footer.

use super::ChatWidget;
use crate::token_usage::TokenUsage;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct Usage {
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<Counts>,
}

#[derive(Serialize)]
struct Counts {
    total_input_tokens: i64,
    cache_read_input_tokens: i64,
    cache_write_input_tokens: i64,
    other_input_tokens: i64,
    output_tokens: i64,
}

impl Counts {
    fn reported(usage: &TokenUsage) -> Option<Self> {
        let other_input_tokens = usage
            .input_tokens
            .checked_sub(usage.cached_input_tokens)?
            .checked_sub(usage.cache_write_input_tokens)?;
        if [
            other_input_tokens,
            usage.cached_input_tokens,
            usage.cache_write_input_tokens,
            usage.output_tokens,
        ]
        .into_iter()
        .any(|count| count < 0)
        {
            return None;
        }
        // A synthetic context fill reports its size without a token breakdown.
        if usage.total_tokens != 0 && usage.input_tokens == 0 && usage.output_tokens == 0 {
            return None;
        }
        Some(Self {
            total_input_tokens: usage.input_tokens,
            cache_read_input_tokens: usage.cached_input_tokens,
            cache_write_input_tokens: usage.cache_write_input_tokens,
            other_input_tokens,
            output_tokens: usage.output_tokens,
        })
    }
}

impl From<&ChatWidget> for Usage {
    fn from(chat: &ChatWidget) -> Self {
        Self {
            total: chat
                .token_info
                .as_ref()
                .and_then(|info| Counts::reported(&info.total_token_usage)),
        }
    }
}
