use crate::error::Result;
use crate::stream::WireEvent;
use crate::stream::WireRateLimitSnapshot;
use crate::stream::WireRateLimitWindow;

pub fn map_response_event_to_wire(ev: crate::stream::ResponseEvent) -> Result<WireEvent> {
    Ok(match ev {
        crate::stream::ResponseEvent::Created => WireEvent::Created,
        crate::stream::ResponseEvent::OutputItemDone(item) => {
            WireEvent::OutputItemDone(serde_json::to_value(item).unwrap_or(serde_json::Value::Null))
        }
        crate::stream::ResponseEvent::OutputItemAdded(item) => WireEvent::OutputItemAdded(
            serde_json::to_value(item).unwrap_or(serde_json::Value::Null),
        ),
        crate::stream::ResponseEvent::Completed {
            response_id,
            token_usage,
        } => {
            let mapped = token_usage.map(|u| crate::stream::WireTokenUsage {
                input_tokens: u.input_tokens,
                cached_input_tokens: u.cached_input_tokens,
                output_tokens: u.output_tokens,
                reasoning_output_tokens: u.reasoning_output_tokens,
                total_tokens: u.total_tokens,
            });
            WireEvent::Completed {
                response_id,
                token_usage: mapped,
            }
        }
        crate::stream::ResponseEvent::OutputTextDelta(s) => WireEvent::OutputTextDelta(s),
        crate::stream::ResponseEvent::ReasoningSummaryDelta(s) => {
            WireEvent::ReasoningSummaryDelta(s)
        }
        crate::stream::ResponseEvent::ReasoningContentDelta(s) => {
            WireEvent::ReasoningContentDelta(s)
        }
        crate::stream::ResponseEvent::ReasoningSummaryPartAdded => {
            WireEvent::ReasoningSummaryPartAdded
        }
        crate::stream::ResponseEvent::RateLimits(s) => {
            let to_win = |w: Option<codex_protocol::protocol::RateLimitWindow>| -> Option<WireRateLimitWindow> {
                w.map(|w| WireRateLimitWindow {
                    used_percent: Some(w.used_percent),
                    window_minutes: w.window_minutes,
                    resets_at: w.resets_at,
                })
            };
            WireEvent::RateLimits(WireRateLimitSnapshot {
                primary: to_win(s.primary),
                secondary: to_win(s.secondary),
            })
        }
    })
}
