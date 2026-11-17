//! Provider-facing usage comes from the same token report as the built-in footer.

use super::ChatWidget;
use crate::token_usage::TokenUsage;
use serde::Serialize;

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", optional_fields)
)]
pub(super) struct Usage {
    context: Context,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<Counts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cost: Option<Cost>,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", optional_fields)
)]
struct Cost {
    #[serde(skip_serializing_if = "Option::is_none")]
    total_cost_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_credits: Option<f64>,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", optional_fields)
)]
struct Context {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(as = "Option<f64>"))]
    context_window_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(as = "Option<f64>"))]
    used_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(as = "Option<f64>"))]
    used_percentage: Option<i64>,
}

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "TokenUsage")
)]
struct Counts {
    #[cfg_attr(test, ts(type = "number"))]
    total_input_tokens: i64,
    #[cfg_attr(test, ts(type = "number"))]
    cache_read_input_tokens: i64,
    #[cfg_attr(test, ts(type = "number"))]
    cache_write_input_tokens: i64,
    #[cfg_attr(test, ts(type = "number"))]
    other_input_tokens: i64,
    #[cfg_attr(test, ts(type = "number"))]
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
        let context_window_size = chat.status_line_context_window_size();
        let units = |micros: i64| (micros >= 0).then_some(micros as f64 / 1_000_000.0);
        let total_cost_usd = chat.status_line_thread_cost_micros().and_then(units);
        let total_credits = chat.status_line_thread_credits_micros().and_then(units);
        Self {
            cost: (total_cost_usd.is_some() || total_credits.is_some()).then_some(Cost {
                total_cost_usd,
                total_credits,
            }),
            context: Context {
                context_window_size,
                used_tokens: chat
                    .token_info
                    .as_ref()
                    .map(|info| info.last_token_usage.tokens_in_context_window()),
                used_percentage: context_window_size
                    .and_then(|_| chat.status_line_context_used_percent()),
            },
            total: chat
                .token_info
                .as_ref()
                .and_then(|info| Counts::reported(&info.total_token_usage)),
        }
    }
}
